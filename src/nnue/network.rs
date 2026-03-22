use super::defs::*;

pub struct Network {
    pub ft_weights: Box<[[i16; HIDDEN_SIZE]; BUCKET_FEATURE_SIZE]>,
    pub ft_biases: [i16; HIDDEN_SIZE],
    /// Transposed L1 weights: stored as [L1_SIZE][HIDDEN_SIZE * 2] for cache-friendly access.
    /// File format uses [HIDDEN_SIZE * 2][L1_SIZE]; transposition happens at load time.
    pub l1_weights: [[i16; HIDDEN_SIZE * 2]; L1_SIZE],
    pub l1_biases: [i16; L1_SIZE],
    pub l2_weights: [i16; L1_SIZE],
    pub l2_bias: i16,
}

impl Network {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 24 {
            return None;
        }
        if data[0..4] != NET_MAGIC {
            return None;
        }

        let version = u32::from_le_bytes(data[4..8].try_into().ok()?);
        if version != NET_VERSION {
            return None;
        }

        let num_buckets = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
        let feature_size = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
        let hidden_size = u32::from_le_bytes(data[16..20].try_into().ok()?) as usize;
        let l1_size = u32::from_le_bytes(data[20..24].try_into().ok()?) as usize;

        if num_buckets != NUM_BUCKETS
            || feature_size != FEATURE_SIZE
            || hidden_size != HIDDEN_SIZE
            || l1_size != L1_SIZE
        {
            return None;
        }

        let expected_size = 24
            + (BUCKET_FEATURE_SIZE * HIDDEN_SIZE + HIDDEN_SIZE + HIDDEN_SIZE * 2 * L1_SIZE + L1_SIZE + L1_SIZE + 1) * 2;
        if data.len() < expected_size {
            return None;
        }

        let mut offset = 24;

        let read_i16 = |offset: &mut usize| -> i16 {
            let val = i16::from_le_bytes([data[*offset], data[*offset + 1]]);
            *offset += 2;
            val
        };

        let ft_weights: Box<[[i16; HIDDEN_SIZE]; BUCKET_FEATURE_SIZE]> = {
            let mut v = vec![[0i16; HIDDEN_SIZE]; BUCKET_FEATURE_SIZE];
            for row in v.iter_mut() {
                for val in row.iter_mut() {
                    *val = read_i16(&mut offset);
                }
            }
            v.into_boxed_slice().try_into().ok()?
        };

        let mut ft_biases = [0i16; HIDDEN_SIZE];
        for val in ft_biases.iter_mut() {
            *val = read_i16(&mut offset);
        }

        // Read L1 weights in file format [HIDDEN_SIZE*2][L1_SIZE] and transpose to [L1_SIZE][HIDDEN_SIZE*2]
        let mut l1_weights = [[0i16; HIDDEN_SIZE * 2]; L1_SIZE];
        #[allow(clippy::needless_range_loop)]
        for row in 0..HIDDEN_SIZE * 2 {
            for col in 0..L1_SIZE {
                l1_weights[col][row] = read_i16(&mut offset);
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
    #[inline(always)]
    pub fn forward(&self, our_acc: &[i16; HIDDEN_SIZE], their_acc: &[i16; HIDDEN_SIZE]) -> i16 {
        let qa = QA as i64;
        let qb = QB as i64;

        // Pre-compute SCReLU activations once (not 32× per neuron)
        let mut activated = [0i32; HIDDEN_SIZE * 2];
        for (j, &val) in our_acc.iter().enumerate() {
            let c = val.clamp(0, QA as i16) as i32;
            activated[j] = c * c;
        }
        for (j, &val) in their_acc.iter().enumerate() {
            let c = val.clamp(0, QA as i16) as i32;
            activated[HIDDEN_SIZE + j] = c * c;
        }

        // L1: simple dot product per neuron (auto-vectorizable)
        let mut l1 = [0i32; L1_SIZE];
        for (i, l1_val) in l1.iter_mut().enumerate() {
            let mut sum = 0i64;
            let weights = &self.l1_weights[i];
            for j in 0..HIDDEN_SIZE * 2 {
                sum += activated[j] as i64 * weights[j] as i64;
            }
            let val = sum / qa + self.l1_biases[i] as i64;
            let clamped = (val / qb).clamp(0, qa) as i32;
            *l1_val = clamped * clamped;
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
    /// Writes L1 weights in original [HIDDEN_SIZE*2][L1_SIZE] layout for file compatibility.
    #[cfg(test)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&NET_MAGIC);
        data.extend_from_slice(&NET_VERSION.to_le_bytes());
        data.extend_from_slice(&(NUM_BUCKETS as u32).to_le_bytes());
        data.extend_from_slice(&(FEATURE_SIZE as u32).to_le_bytes());
        data.extend_from_slice(&(HIDDEN_SIZE as u32).to_le_bytes());
        data.extend_from_slice(&(L1_SIZE as u32).to_le_bytes());

        for row in self.ft_weights.iter() {
            for &val in row {
                data.extend_from_slice(&val.to_le_bytes());
            }
        }

        for &val in &self.ft_biases {
            data.extend_from_slice(&val.to_le_bytes());
        }

        // Write in original [HIDDEN_SIZE*2][L1_SIZE] layout (un-transpose)
        for row in 0..HIDDEN_SIZE * 2 {
            for col in 0..L1_SIZE {
                data.extend_from_slice(&self.l1_weights[col][row].to_le_bytes());
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

    pub fn zero_network() -> Network {
        Network {
            ft_weights: vec![[0i16; HIDDEN_SIZE]; BUCKET_FEATURE_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            ft_biases: [0i16; HIDDEN_SIZE],
            l1_weights: [[0i16; HIDDEN_SIZE * 2]; L1_SIZE],
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

    #[test]
    fn known_position_eval_sanity() {
        use std::rc::Rc;

        use crate::bitboards::Bitboards;
        use crate::hash::Hasher;
        use crate::nnue::NnueEval;
        use crate::position::Position;
        use crate::search::defs::FEN_START_POSITION;

        let mut nnue = NnueEval::from_bytes(crate::EMBEDDED_NET).expect("embedded NNUE net is invalid");
        let bitboards = Rc::new(Bitboards::new());
        let hasher = Rc::new(Hasher::new());
        let mut position = Position::new(bitboards, hasher);

        // Test 1: starting position should be roughly balanced
        position.set(FEN_START_POSITION.to_string());
        nnue.refresh(&position);
        let eval = nnue.evaluate(position.side_to_move);
        assert!(
            eval > -200 && eval < 200,
            "Starting position eval {} cp is out of expected range [-200, +200]",
            eval
        );

        // Test 2: white up a queen should be clearly positive
        // FEN: 4k3/8/8/8/8/8/8/4KQ2 w - - 0 1
        position.set("4k3/8/8/8/8/8/8/4KQ2 w - - 0 1".to_string());
        nnue.refresh(&position);
        let eval_queen_up = nnue.evaluate(position.side_to_move);
        assert!(
            eval_queen_up > 0,
            "White up a queen should be positive, got {} cp",
            eval_queen_up
        );

        // Test 3: black up a queen (flip), eval from side-to-move (white) should be negative
        position.set("4kq2/8/8/8/8/8/8/4K3 w - - 0 1".to_string());
        nnue.refresh(&position);
        let eval_queen_down = nnue.evaluate(position.side_to_move);
        assert!(
            eval_queen_down < 0,
            "White down a queen should be negative, got {} cp",
            eval_queen_down
        );
    }
}
