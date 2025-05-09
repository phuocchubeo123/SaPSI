use crate::types::Pair;
use crate::error::{Error, Result};
use crate::utils::{radix_sort, MASK};
use std::convert::TryInto;
use std::ops::{BitXor, Shl, Shr};
use sp_core::U256;
use blake3;

const EPSILON: f64 = 1.5; // can change
const BAND_WIDTH: usize = 200; // can change
pub struct RbOkvsF2k {
    pub columns: usize,
    band_width: usize,
    r1: [u8; 16],
    r2: [u8; 16],
}

impl RbOkvsF2k {
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
        self.simple_gauss(y, matrix, start_pos, self.band_width)
    }

    pub fn decode(&self, encoding: &Vec<u128>, key: &[u128]) -> Vec<u128> {
        let n = key.len();
        let mut start = vec![0usize; n];
        let mut band = vec![U256::default(); n];

        start.iter_mut().enumerate().for_each(|(i, start_i)| {
            *start_i = hash_to_index(key[i], &self.r1, self.columns - self.band_width);
        });

        band.iter_mut().enumerate().for_each(|(i, band_i)| {
            *band_i = hash_to_band(key[i], &self.r2);
        });

        let mut res = vec![0; n];

        for i in 0..n {
            for j in 0..self.band_width {
                if band[i].0[j / 64] & MASK[j & 63] != 0 {
                    res[i] ^= encoding[start[i] + j];
                }
            }
        }

        res
    }

    fn create_sorted_matrix(&self, input: &Vec<Pair<u128, u128>>) -> Result<(Vec<U256>, Vec<usize>, Vec<u128>)> {
        let n = input.len();
        let mut start_pos: Vec<(usize, usize)> = vec![(0, 0); n];
        let mut matrix: Vec<U256> = vec![U256::default(); n];
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
        band_width: usize,
    ) -> Result<Vec<u128>> {
        let rows = bands.len();

        assert_eq!(rows, start_pos.len());
        assert_eq!(rows, y.len());

        let mut bands_bool = vec![vec![false; band_width]; rows];

        for i in 0..rows {
            for j in 0..4 {
                for k in 0..64 {
                    if j * 64 + k >= band_width {
                        break;
                    }
                    if bands[i].0[j] & MASK[k] != 0 {
                        bands_bool[i][j * 64 + k] = true;
                    }
                }
            }
        }

        let mut pivot = vec![0 as usize; rows];
        let mut first_nonzero = vec![band_width; rows];

        for i in 0..rows {
            let y_i = y[i];
            for j in 0..band_width {
                if bands_bool[i][j] {
                    first_nonzero[i] = j;
                    break;
                }
            }

            if first_nonzero[i] == band_width {
                return Err(Error::ZeroRow(i));
            }

            pivot[i] = first_nonzero[i] + start_pos[i];

            let bands_bool_i = bands_bool[i].clone();   

            for j in (i + 1)..rows {
                if start_pos[j] > pivot[i] {
                    break;
                }
                let offset = pivot[i] - start_pos[j];
                println!("offset: {}", offset);
                let lead = bands_bool[j][offset];
                if lead {
                    for k in 0..(band_width - first_nonzero[i]) {
                        bands_bool[j][k + offset] ^= bands_bool_i[k + first_nonzero[i]];
                    }
                    y[j] ^= y_i;
                }

            }
        }

        println!("bands: {:?}", bands);

        let mut x = vec![0; self.columns];
        for i in (0..rows).rev() {
            let mut res = y[i];   
            for j in 0..band_width {
                if bands_bool[i][j] {
                    res ^= x[start_pos[i] + j];
                }
            }
            x[pivot[i]] = res;
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