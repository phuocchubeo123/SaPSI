pub fn gf128mul(x: u128, y: u128) -> u128 {
    const POLY: u128 = 0b10000111; // x^128 + x^7 + x^2 + x + 1
    let mut x_shifted = x;
    let mut res: u128 = 0;

    for bit in 0..128 {
        if y & (1 << bit) != 0 {
            res ^= x_shifted; 
        }
        if x_shifted & (1 << 127) != 0 {
            x_shifted = (x_shifted << 1) ^ POLY;
        } else {
            x_shifted <<= 1;
        }
    }
    res
}

pub fn uni_hash_coeff_gen_f2k(coeff: &mut [u128], seed: u128, sz: usize) {
    if sz == 0 {
        return;
    }

    coeff[0] = seed.clone();
    for i in 1..sz {
        coeff[i] = gf128mul(coeff[i-1], seed);
    }
}

pub fn vector_inner_product_f2k(vec1: &[u128], vec2: &[u128]) -> u128 {
    vec1.iter()
        .zip(vec2)
        .fold(0, |acc, (v1, v2)| acc ^ gf128mul(*v1, *v2))
}