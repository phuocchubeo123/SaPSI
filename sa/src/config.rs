pub const DIMENSION: usize = 4;
pub const DIMENSION2: usize = DIMENSION * 2; // As we need 2 range checks for each dimension
pub const RADIUS: usize = 120;
pub const RANGE_BITS: usize = 9; // RANGE = 2^RANGE_BITS
pub const LOC_FUNC_COUNT: usize = 3;
pub const PREF_LENGTH: &[usize] = &[2, 4, 6, 9];
pub const N: usize = 1 << 8;