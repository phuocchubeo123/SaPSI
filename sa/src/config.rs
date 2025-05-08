pub const DIMENSION: usize = 2;
pub const DIMENSION2: usize = DIMENSION * 2; // As we need 2 range checks for each dimension
pub const RADIUS: usize = 10;
pub const RANGE_BITS: usize = 6; // RANGE = 2^RANGE_BITS
pub const LOC_FUNC_COUNT: usize = 3;
pub const PREF_CUT: usize = 6; // The number of bits to cut off from the prefix
pub const PREF_LENGTH: &[usize] = &[1, 2, 3, 4, 5, 6];
pub const N: usize = 1 << 5;