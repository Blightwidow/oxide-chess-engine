/// 768 input features: 2 colors × 6 piece types × 64 squares
pub const FEATURE_SIZE: usize = 768;
/// Hidden layer size of the feature transformer accumulator
pub const HIDDEN_SIZE: usize = 256;
/// Second hidden layer size
pub const L1_SIZE: usize = 32;
/// Quantization factor for accumulator clipped ReLU
pub const QA: i32 = 255;
/// Quantization factor for output layer
pub const QB: i32 = 64;
/// Scale factor to convert network output to centipawns
pub const SCALE: i32 = 400;
/// Network file format version
pub const NET_VERSION: u32 = 1;
/// Magic bytes for the .nnue file header
pub const NET_MAGIC: [u8; 4] = *b"OXNN";
