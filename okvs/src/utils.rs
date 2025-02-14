use crate::error::{Error, Result};

use sha3::{Digest, Sha3_256};
use sp_core::U256;
use lambdaworks_math::field::fields::fft_friendly::stark_252_prime_field::Stark252PrimeField;
use lambdaworks_math::field::element::FieldElement;
use lambdaworks_math::unsigned_integer::element::UnsignedInteger;
use rand::random;
use std::time::Instant; 
use rayon::prelude::*;
use sp_core::sp_std::cmp::min;

pub type F = Stark252PrimeField;
pub type FE = FieldElement<F>;

pub fn rand_field_element() -> FE {
    let rand_big = UnsignedInteger { limbs: random() };
    FE::new(rand_big)
}


pub fn hash_to_index(x: FE, r1: &[u8], range: usize) -> usize {
    let mut hasher = Sha3_256::new();
    hasher.update(x.to_bytes_le());
    hasher.update(r1);
    let mut res = [0u8; 8];
    res.copy_from_slice(&hasher.finalize()[..8]);
    usize::from_le_bytes(res) % range
}

pub fn hash_to_band(x: FE, r2: &[u8]) -> U256 {
    let mut hasher = Sha3_256::new();
    hasher.update(x.to_bytes_le());
    hasher.update(r2);
    let mut res = [0u8; 32];
    res.copy_from_slice(&hasher.finalize());
    U256::from_little_endian(&res)
}

pub fn simple_gauss(
    mut y: Vec<FE>,
    mut bands: Vec<U256>,
    start_pos: Vec<usize>,
    cols: usize,
    band_width: usize,
) -> Result<Vec<FE>> {

    let rows = bands.len();

    assert_eq!(rows, start_pos.len());
    assert_eq!(rows, y.len());
    let mut pivot = vec![0 as usize; rows];

    let mut bands_FE = vec![vec![FE::zero(); band_width]; rows];

    for i in 0..rows {
        for j in 0..4 {
            for k in 0..64 {
                if j * 64 + k >= band_width {
                    break;
                }
                if bands[i].0[j] & MASK[k] != 0 {
                    bands_FE[i][j*64+k] = FE::one();
                }
            }
        }
    }

    let mut first_nonzero = vec![band_width; rows];
    let mut skip_num = 0;

    for i in 0..rows {
        let y_i = y[i].clone();

        for j in 0..band_width {
            if bands_FE[i][j] != FE::zero() {
                first_nonzero[i] = j;
                break;
            }
        }

        if first_nonzero[i] == band_width {
            return Err(Error::ZeroRow(i));
        }

        pivot[i] = first_nonzero[i] + start_pos[i];

        // Scan first to see if we need to do inverse now
        let mut cnt = 0;
        for j in (i + 1)..rows {
            if start_pos[j] > pivot[i] {
                break;
            }
            cnt += 1;
        }

        if cnt == 0 {
            skip_num += 1;
            continue;
        }

        let lead_inv = bands_FE[i][first_nonzero[i]].inv().unwrap();

        for j in first_nonzero[i]..band_width {
            bands_FE[i][j] = bands_FE[i][j] * lead_inv;
        }
        y[i] = y[i] * lead_inv;


        let bands_FE_i = &bands_FE[i].to_vec();

        for j in (i + 1)..rows {
            if start_pos[j] > pivot[i] {
                break;
            }
            let offset = pivot[i] - start_pos[j];
            if bands_FE[j][pivot[i] - start_pos[j]] != FE::zero() {
                let lead = bands_FE[j][offset];

                for k in (0..(band_width-first_nonzero[i]-7)).step_by(8) {
                    bands_FE[j][k + offset] = bands_FE[j][k + offset] - lead * bands_FE[i][k + first_nonzero[i]];
                    bands_FE[j][k + offset + 1] = bands_FE[j][k + offset + 1] - lead * bands_FE[i][k + first_nonzero[i] + 1];
                    bands_FE[j][k + offset + 2] = bands_FE[j][k + offset + 2] - lead * bands_FE[i][k + first_nonzero[i] + 2];
                    bands_FE[j][k + offset + 3] = bands_FE[j][k + offset + 3] - lead * bands_FE[i][k + first_nonzero[i] + 3];
                    bands_FE[j][k + offset + 4] = bands_FE[j][k + offset + 4] - lead * bands_FE[i][k + first_nonzero[i] + 4];
                    bands_FE[j][k + offset + 5] = bands_FE[j][k + offset + 5] - lead * bands_FE[i][k + first_nonzero[i] + 5];
                    bands_FE[j][k + offset + 6] = bands_FE[j][k + offset + 6] - lead * bands_FE[i][k + first_nonzero[i] + 6];
                    bands_FE[j][k + offset + 7] = bands_FE[j][k + offset + 7] - lead * bands_FE[i][k + first_nonzero[i] + 7];
                }
                
                let remain = (band_width - first_nonzero[i]) % 8;
                for k in (band_width - first_nonzero[i] - remain)..(band_width - first_nonzero[i]) {
                    bands_FE[j][k + offset] = bands_FE[j][k + offset] - lead * bands_FE[i][k + first_nonzero[i]];
                }

                y[j] = y[j] - lead * y[i];
            }
        }
    }

    println!("Number of leading coefficients skipped: {}", skip_num);

    // clean up rows with non_unit leading coeffiicients
    bands_FE.par_iter_mut().zip(y.par_iter_mut()).enumerate().for_each(|(i, (band_i, y_i))| {
        let lead = band_i[first_nonzero[i]];
        if lead != FE::one() {
            let lead_inv = lead.inv().unwrap();

            for j in first_nonzero[i]..band_width {
                band_i[j] = band_i[j] * lead_inv;
            }
            *y_i = *y_i * lead_inv;
        }
    });

    // back substitution
    let mut x = vec![FE::zero(); cols];
    for i in (0..rows).rev() {
        let mut res = y[i];
        for j in 0..band_width {
            if (x[start_pos[i] + j] == FE::zero()) || (bands_FE[i][j] == FE::zero()) {
                continue;
            }
            res = res - bands_FE[i][j] * x[start_pos[i]+j];
        }
        x[pivot[i]] = res;
    }

    Ok(x)
}


const MASK: [u64; 64] = [
    0x1,
    0x2,
    0x4,
    0x8,
    0x10,
    0x20,
    0x40,
    0x80,
    0x100,
    0x200,
    0x400,
    0x800,
    0x1000,
    0x2000,
    0x4000,
    0x8000,
    0x10000,
    0x20000,
    0x40000,
    0x80000,
    0x100000,
    0x200000,
    0x400000,
    0x800000,
    0x1000000,
    0x2000000,
    0x4000000,
    0x8000000,
    0x10000000,
    0x20000000,
    0x40000000,
    0x80000000,
    0x100000000,
    0x200000000,
    0x400000000,
    0x800000000,
    0x1000000000,
    0x2000000000,
    0x4000000000,
    0x8000000000,
    0x10000000000,
    0x20000000000,
    0x40000000000,
    0x80000000000,
    0x100000000000,
    0x200000000000,
    0x400000000000,
    0x800000000000,
    0x1000000000000,
    0x2000000000000,
    0x4000000000000,
    0x8000000000000,
    0x10000000000000,
    0x20000000000000,
    0x40000000000000,
    0x80000000000000,
    0x100000000000000,
    0x200000000000000,
    0x400000000000000,
    0x800000000000000,
    0x1000000000000000,
    0x2000000000000000,
    0x4000000000000000,
    0x8000000000000000,
];


pub fn xor(a: U256, b: U256, start_a: usize, start_b: usize) -> U256{
    match start_a.cmp(&start_b) {
        std::cmp::Ordering::Equal => b ^ a,
        std::cmp::Ordering::Less => {
            let diff = start_b - start_a;
            ((b >> diff) ^ a) << diff
        }
        std::cmp::Ordering::Greater => {
            let diff = start_b - start_a;
            ((b << diff) ^ a) << diff
        }
    }
}


pub fn inner_product(m: &U256, x: &[FE]) -> FE {
    let mut result = FE::zero();
    let mut bits = m.bits();
    if x.len() < bits {
        bits = x.len();
    }
    // println!("bits: {}", bits);

    if bits <= 64 {
        for i in 0..bits {
            if m.0[0] & MASK[i] != 0 {
                result += x[i];
            }
        }
        return result;
    }


    for i in 0..64 {
        if m.0[0] & MASK[i] != 0 {
            result += x[i];
        }
    }

    let x64 = &x[64..];

    if bits <= 128 {
        for i in 0..bits-64 {
            if m.0[1] & MASK[i] != 0 {
                result += x64[i];
            }
        }
        return result;
    }

    for i in 0..64 {
        if m.0[1] & MASK[i] != 0 {
            result += x64[i];
        }
    }

    let x128 = &x[128..];

    if bits <= 192 {
        for i in 0..bits - 128 {
            if m.0[2] & MASK[i] != 0 {
                result += x128[i];
            }
        }
        return result;
    }

    for i in 0..64 {
        if m.0[2] & MASK[i] != 0 {
            result += x128[i];
        }
    }

    let x192 = &x[192..];

    for i in 0..bits - 192 {
        if m.0[3] & MASK[i] != 0 {
            result += x192[i];
        }
    }
    result
}


/// Sort by arr[i].1
pub fn radix_sort(arr: &mut Vec<(usize, usize)>, max: usize) {
    let mut exp = 1;
    loop {
        if max / exp == 0 {
            break;
        }
        *arr = count_sort(arr, exp);
        exp *= 10;
    }
}

fn count_sort(arr: &Vec<(usize, usize)>, exp: usize) -> Vec<(usize, usize)> {
    let mut count = [0usize; 10];

    arr.iter().for_each(|(_, b)| count[(b / exp) % 10] += 1);

    for i in 1..10 {
        count[i] += count[i - 1];
    }

    let mut output = vec![(0usize, 0usize); arr.len()];

    arr.iter().rev().for_each(|(a, b)| {
        output[count[(b / exp) % 10] - 1] = (*a, *b);
        count[(b / exp) % 10] -= 1;
    });

    output
}
