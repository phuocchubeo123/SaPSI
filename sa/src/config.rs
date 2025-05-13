pub const DIMENSION: usize = 4;
pub const DIMENSION2: usize = DIMENSION * 2; // As we need 2 range checks for each dimension
pub const RADIUS: usize = 250;
pub const RANGE_BITS: usize = 10; // RANGE = 2^RANGE_BITS
pub const LOC_FUNC_COUNT: usize = 3;
pub const PREF_LENGTH: &[usize] = &[2, 4, 6, 8, 10];
pub const N: usize = 1 << 16;