use super::defs::*;

pub struct Network {
    pub ft_weights: Vec<[i16; HIDDEN_SIZE]>,
    pub ft_biases: [i16; HIDDEN_SIZE],
    pub l1_weights: Vec<[i16; L1_SIZE]>,
    pub l1_biases: [i16; L1_SIZE],
    pub l2_weights: [i16; L1_SIZE],
    pub l2_bias: i16,
}

impl Network {
    pub fn zeroed() -> Self {
        Self {
            ft_weights: vec![[0i16; HIDDEN_SIZE]; FEATURE_SIZE],
            ft_biases: [0i16; HIDDEN_SIZE],
            l1_weights: vec![[0i16; L1_SIZE]; HIDDEN_SIZE * 2],
            l1_biases: [0i16; L1_SIZE],
            l2_weights: [0i16; L1_SIZE],
            l2_bias: 0,
        }
    }

    pub fn load(path: &str) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        Self::from_bytes(&data)
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }
        if data[0..4] != NET_MAGIC {
            return None;
        }

        let version = u32::from_le_bytes(data[4..8].try_into().ok()?);
        if version != NET_VERSION {
            return None;
        }

        let feature_size = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
        let hidden_size = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
        let l1_size = u32::from_le_bytes(data[16..20].try_into().ok()?) as usize;

        if feature_size != FEATURE_SIZE || hidden_size != HIDDEN_SIZE || l1_size != L1_SIZE {
            return None;
        }

        let expected_size =
            20 + (FEATURE_SIZE * HIDDEN_SIZE + HIDDEN_SIZE + HIDDEN_SIZE * 2 * L1_SIZE + L1_SIZE + L1_SIZE + 1) * 2;
        if data.len() < expected_size {
            return None;
        }

        let mut offset = 20;

        let read_i16 = |offset: &mut usize| -> i16 {
            let val = i16::from_le_bytes([data[*offset], data[*offset + 1]]);
            *offset += 2;
            val
        };

        let mut ft_weights = vec![[0i16; HIDDEN_SIZE]; FEATURE_SIZE];
        for row in ft_weights.iter_mut() {
            for val in row.iter_mut() {
                *val = read_i16(&mut offset);
            }
        }

        let mut ft_biases = [0i16; HIDDEN_SIZE];
        for val in ft_biases.iter_mut() {
            *val = read_i16(&mut offset);
        }

        let mut l1_weights = vec![[0i16; L1_SIZE]; HIDDEN_SIZE * 2];
        for row in l1_weights.iter_mut() {
            for val in row.iter_mut() {
                *val = read_i16(&mut offset);
            }
        }

        let mut l1_biases = [0i16; L1_SIZE];
        for val in l1_biases.iter_mut() {
            *val = read_i16(&mut offset);
        }

        let mut l2_weights = [0i16; L1_SIZE];
        for val in l2_weights.iter_mut() {
            *val = read_i16(&mut offset);
        }

        let l2_bias = read_i16(&mut offset);

        Some(Self {
            ft_weights,
            ft_biases,
            l1_weights,
            l1_biases,
            l2_weights,
            l2_bias,
        })
    }

    /// Run the forward pass from accumulators to centipawn output.
    ///
    /// Architecture: SCReLU(accumulator) -> L1(32) with SCReLU -> L2(1) -> scale
    ///
    /// Quantization: FT weights/biases at QA, L1/L2 weights at QB, L1/L2 biases at QA*QB.
    /// SCReLU squares values, creating QA²-scale intermediates. We divide by QA after each
    /// dot product to keep values at QA*QB scale, then divide by QB for the next SCReLU.
    /// Uses i64 accumulation to avoid overflow from squared values × 512 inputs.
    pub fn forward(&self, our_acc: &[i16; HIDDEN_SIZE], their_acc: &[i16; HIDDEN_SIZE]) -> i16 {
        let qa = QA as i64;
        let qb = QB as i64;

        // Layer 1: SCReLU(accumulator) -> L1_SIZE with SCReLU
        let mut l1 = [0i32; L1_SIZE];
        for (i, l1_val) in l1.iter_mut().enumerate() {
            let mut sum = 0i64;
            for (j, &acc_val) in our_acc.iter().enumerate() {
                let c = acc_val.clamp(0, QA as i16) as i64;
                sum += c * c * self.l1_weights[j][i] as i64;
            }
            for (j, &acc_val) in their_acc.iter().enumerate() {
                let c = acc_val.clamp(0, QA as i16) as i64;
                sum += c * c * self.l1_weights[HIDDEN_SIZE + j][i] as i64;
            }
            // sum at QA²·QB, divide by QA → QA·QB, add bias (at QA·QB)
            let val = sum / qa + self.l1_biases[i] as i64;
            // Divide by QB → QA scale, clamp to [0, QA], square for SCReLU
            let clamped = (val / qb) as i32;
            let clamped = clamped.clamp(0, QA);
            *l1_val = clamped * clamped; // SCReLU: [0, QA²]
        }

        // Layer 2: L1_SIZE -> 1
        let mut output = 0i64;
        for (i, &l1_val) in l1.iter().enumerate() {
            output += l1_val as i64 * self.l2_weights[i] as i64;
        }
        // output at QA²·QB, divide by QA → QA·QB, add bias (at QA·QB)
        output = output / qa + self.l2_bias as i64;

        // Scale to centipawns
        (output * SCALE as i64 / (qa * qb)) as i16
    }

    /// Serialize network weights to binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&NET_MAGIC);
        data.extend_from_slice(&NET_VERSION.to_le_bytes());
        data.extend_from_slice(&(FEATURE_SIZE as u32).to_le_bytes());
        data.extend_from_slice(&(HIDDEN_SIZE as u32).to_le_bytes());
        data.extend_from_slice(&(L1_SIZE as u32).to_le_bytes());

        for row in &self.ft_weights {
            for &val in row {
                data.extend_from_slice(&val.to_le_bytes());
            }
        }

        for &val in &self.ft_biases {
            data.extend_from_slice(&val.to_le_bytes());
        }

        for row in &self.l1_weights {
            for &val in row {
                data.extend_from_slice(&val.to_le_bytes());
            }
        }

        for &val in &self.l1_biases {
            data.extend_from_slice(&val.to_le_bytes());
        }

        for &val in &self.l2_weights {
            data.extend_from_slice(&val.to_le_bytes());
        }

        data.extend_from_slice(&self.l2_bias.to_le_bytes());

        data
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn zero_network() -> Network {
        Network {
            ft_weights: vec![[0i16; HIDDEN_SIZE]; FEATURE_SIZE],
            ft_biases: [0i16; HIDDEN_SIZE],
            l1_weights: vec![[0i16; L1_SIZE]; HIDDEN_SIZE * 2],
            l1_biases: [0i16; L1_SIZE],
            l2_weights: [0i16; L1_SIZE],
            l2_bias: 0,
        }
    }

    #[test]
    fn zero_network_returns_zero() {
        let net = zero_network();
        let our_acc = [0i16; HIDDEN_SIZE];
        let their_acc = [0i16; HIDDEN_SIZE];
        assert_eq!(net.forward(&our_acc, &their_acc), 0);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut net = zero_network();
        net.ft_biases[0] = 42;
        net.l1_weights[0][0] = 10;
        net.l2_weights[0] = 5;
        net.l2_bias = 7;

        let bytes = net.to_bytes();
        let loaded = Network::from_bytes(&bytes).expect("should parse");

        assert_eq!(loaded.ft_biases[0], 42);
        assert_eq!(loaded.l1_weights[0][0], 10);
        assert_eq!(loaded.l2_weights[0], 5);
        assert_eq!(loaded.l2_bias, 7);
    }

    #[test]
    fn rejects_invalid_magic() {
        let mut bytes = zero_network().to_bytes();
        bytes[0] = b'X';
        assert!(Network::from_bytes(&bytes).is_none());
    }

    #[test]
    fn rejects_truncated_data() {
        let bytes = zero_network().to_bytes();
        assert!(Network::from_bytes(&bytes[..bytes.len() - 1]).is_none());
    }
}
