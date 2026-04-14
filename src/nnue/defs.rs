use std::ops::{Deref, DerefMut};

/// 768 input features per bucket: 2 colors × 6 piece types × 64 squares
pub const FEATURE_SIZE: usize = 768;
/// Number of king buckets (one per rank, with horizontal mirroring)
pub const NUM_BUCKETS: usize = 8;
/// Total feature transformer input size: buckets × features per bucket
pub const BUCKET_FEATURE_SIZE: usize = NUM_BUCKETS * FEATURE_SIZE;
/// Hidden layer size of the feature transformer accumulator
pub const HIDDEN_SIZE: usize = 384;
/// Second hidden layer size
pub const L1_SIZE: usize = 32;
/// Quantization factor for accumulator clipped ReLU
pub const QA: i32 = 255;
/// Quantization factor for output layer
pub const QB: i32 = 64;
/// Scale factor to convert network output to centipawns
pub const SCALE: i32 = 400;
/// Network file format version
pub const NET_VERSION: u32 = 3;
/// Magic bytes for the .nnue file header
pub const NET_MAGIC: [u8; 4] = *b"OXNN";

/// 32-byte aligned accumulator for AVX2 SIMD operations.
#[repr(C, align(32))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Accumulator {
    pub data: [i16; HIDDEN_SIZE],
}

impl Accumulator {
    pub const fn zeroed() -> Self {
        Self {
            data: [0i16; HIDDEN_SIZE],
        }
    }
}

impl Deref for Accumulator {
    type Target = [i16; HIDDEN_SIZE];
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for Accumulator {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
