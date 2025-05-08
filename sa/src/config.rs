pub const DIMENSION: usize = 2;
pub const DIMENSION2: usize = DIMENSION * 2; // As we need 2 range checks for each dimension
pub const RADIUS: usize = 30;
pub const RANGE_BITS: usize = 7; // RANGE = 2^RANGE_BITS
pub const LOC_FUNC_COUNT: usize = 3;
pub const PREF_LENGTH: &[usize] = &[2, 4, 7];
pub const N: usize = 1 << 12;