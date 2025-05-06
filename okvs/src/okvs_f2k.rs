use crate::types::Pair;
use crate::error::{Error, Result};
use crate::utils::{radix_sort, MASK};
use std::convert::TryInto;
use std::ops::{BitXor, Shl, Shr};
use sp_core::U256;
use blake3;

const EPSILON: f64 = 1.0; // can change
const BAND_WIDTH: usize = 80; // can change
pub struct RbOkvsF2 {
    pub columns: usize,
    band_width: usize,
    r1: [u8; 16],
    r2: [u8; 16],
}

impl RbOkvsF2 {
    pub fn new(kv_count: usize, r1: &[u8; 16], r2: &[u8; 16]) -> Self {
        let columns = ((1.0 + EPSILON) * kv_count as f64) as usize;

        Self {
            columns,
            band_width: if BAND_WIDTH < columns {
                BAND_WIDTH
            } else {
                columns * 80 / 100
            },
            r1: *r1,
            r2: *r2,
        }
    }

    pub fn encode(&self, input: &Vec<Pair<u128, u128>>) -> Result<Vec<u128>> {
        let (matrix, start_pos, y) = self.create_sorted_matrix(input)?;
        self.simple_gauss(y, matrix, start_pos, self.columns, self.band_width)
    }

    fn create_sorted_matrix(&self, input: &Vec<Pair<u128, u128>>) -> Result<(Vec<U256>, Vec<usize>, Vec<u128>)> {
        let n = input.len();
        let mut start_pos: Vec<(usize, usize)> = vec![(0, 0); n];
        let mut matrix: Vec<U256> = vec![U256::default(); n * self.band_width];
        let mut start_ids: Vec<usize> = vec![0; n];
        let mut y: Vec<u128> = vec![0; n];

        start_pos.iter_mut().enumerate().for_each(|(i, start_pos_i)| {
            *start_pos_i = (i, hash_to_index(input[i].0, &self.r1, self.columns - self.band_width));
        });

        radix_sort(&mut start_pos, self.columns - self.band_width - 1);

        matrix.iter_mut().enumerate().for_each(|(i, matrix_i)| {
            *matrix_i = hash_to_band(input[i].0, &self.r2);
        });

        y.iter_mut().enumerate().for_each(|(i, y_i)| {
            *y_i = input[start_pos[i].0].1.to_owned();
        });

        start_ids.iter_mut().enumerate().for_each(|(i, start_ids_i)| {
            *start_ids_i = start_pos[i].1;
        });

        Ok((matrix, start_ids, y))
    }

    fn simple_gauss(
        &self,
        mut y: Vec<u128>,
        mut bands: Vec<U256>,
        start_pos: Vec<usize>,
        cols: usize,
        band_width: usize,
    ) -> Result<Vec<u128>> {
        let rows = bands.len();

        assert_eq!(rows, start_pos.len());
        assert_eq!(rows, y.len());

        let mut pivot = vec![0 as usize; rows];
        let mut first_nonzero = vec![band_width; rows];

        for i in 0..rows {
            for j in 0..4 {
                for k in 0..64 {
                    if j * 64 + k >= band_width {
                        break;
                    }
                    let mut found_nonzero = false;
                    if bands[i].0[j] & MASK[k] != 0 {
                        first_nonzero[i] = j * 64 + k;
                        found_nonzero = true;
                        break;
                    }
                    if found_nonzero {
                        break;
                    }
                }
            }

            if first_nonzero[i] == band_width {
                return Err(Error::ZeroRow(i));
            }

            pivot[i] = first_nonzero[i] + start_pos[i];
            let bands_i = bands[i].clone();

            for j in (i + 1)..rows {
                if start_pos[j] > pivot[i] {
                    break;
                }
                let offset = pivot[i] - start_pos[j];
                if bands[j].0[offset / 64] & MASK[offset & 63] != 0 {
                    let new_bands_j = bands[j].shl(offset).bitxor(bands_i.shl(first_nonzero[i]));
                    bands[j] = bands[j].bitxor(bands[j].shl(offset).shr(offset)).bitxor(new_bands_j);
                }
                y[j] = y[j] ^ y[i];
            }
        }

        let mut x = vec![0; rows];
        for i in (0..rows).rev() {
            let mut res = y[i];   
            for j in 0..band_width {
                if bands[i].0[j / 64] & MASK[j & 63] != 0 {
                    res = res ^ x[start_pos[i] + j];
                }
            }
        }

        Ok(x)
    }
}

fn hash_to_index(x: u128, r1: &[u8; 16], columns: usize) -> usize {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&x.to_le_bytes());
    hasher.update(r1);
    let hash = hasher.finalize();
    let index = u128::from_le_bytes(hash.as_bytes()[0..16].try_into().unwrap()) % (columns as u128);
    index as usize
}

fn hash_to_band(x: u128, r2: &[u8; 16]) -> U256 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&x.to_le_bytes());
    hasher.update(r2);
    let hash = hasher.finalize();
    U256::from_little_endian(hash.as_bytes())
}