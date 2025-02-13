use aes::{Aes128, Aes256};
use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};
use aes::cipher::consts::U16;
use rand::Rng;
use lambdaworks_math::field::fields::fft_friendly::stark_252_prime_field::Stark252PrimeField;
use lambdaworks_math::field::element::FieldElement;
use lambdaworks_math::traits::ByteConversion;

pub type F = Stark252PrimeField;
pub type FE = FieldElement<F>;

pub struct PRP {
    key: [u8; 16],
    aes: Aes128,
}

impl PRP {
    pub fn new(key: Option<&[u8; 16]>) -> Self {
        let mut aes_key = [0u8; 16];
        if let Some(k) = key {
            aes_key.copy_from_slice(k);
        } else {
            let mut rng = rand::thread_rng();
            rng.fill(&mut aes_key);
        }

        let aes = Aes128::new(GenericArray::from_slice(&aes_key));

        PRP {
            key: aes_key,
            aes: aes,
        }
    }

    pub fn permute_block(&self, data: &mut [[u8; 16]], nblocks: usize) {
        let mut aes_block: Vec<_> = data
            .iter()
            .map(|x| GenericArray::clone_from_slice(x))
            .collect();
        self.aes.encrypt_blocks(&mut aes_block);
        for (i, encrypted) in aes_block.iter().enumerate() {
            data[i].copy_from_slice(encrypted);
        }
    }
}

pub struct FieldPRP {
    key: [u8; 32],
}

impl FieldPRP {
    pub fn new(seeds: Option<&[u8; 32]>) -> Self {
        let mut prp_key = [0u8; 32];
        if let Some(s) = seeds {
            prp_key.copy_from_slice(s);
        } else {
            let mut rng = rand::thread_rng();
            rng.fill(&mut prp_key);
        }

        FieldPRP {
            key: prp_key,
        }
    }

    pub fn permute_u128_to_FE(&self, data: &[[u8; 16]], out: &mut [FE], nblocks: usize) {
        let aes_block = [
            GenericArray::<u8, U16>::clone_from_slice(&self.key[..16]),
            GenericArray::<u8, U16>::clone_from_slice(&self.key[16..]),
        ];

        for (i, block) in data.iter().enumerate() {
            let aes_key = Aes128::new(GenericArray::from_slice(block));
            let mut tmp = [aes_block[0].clone(), aes_block[1].clone()];
            aes_key.encrypt_blocks(&mut tmp);

            out[i] = FE::from_bytes_le(&[
                tmp[0][0], tmp[0][1], tmp[0][2], tmp[0][3], tmp[0][4], tmp[0][5], tmp[0][6], tmp[0][7],
                tmp[0][8], tmp[0][9], tmp[0][10], tmp[0][11], tmp[0][12], tmp[0][13], tmp[0][14], tmp[0][15],
                tmp[1][0], tmp[1][1], tmp[1][2], tmp[1][3], tmp[1][4], tmp[1][5], tmp[1][6], tmp[1][7],
                tmp[1][8], tmp[1][9], tmp[1][10], tmp[1][11], tmp[1][12], tmp[1][13], tmp[1][14], tmp[1][15]
                ]).expect("Failed to convert random FE.");
        }
    }

    // pub fn permute_FE_to_usize(&self, data: &[FE]) -> Vec<usize> {
    //     let nblocks = data.len();
        
    // }

    pub fn permute_block(&self, data: &mut [FE], nblocks: usize) {
        let aes_block = [
            GenericArray::<u8, U16>::clone_from_slice(&self.key[..16]),
            GenericArray::<u8, U16>::clone_from_slice(&self.key[16..]),
        ];

        // let mut encrypted_block = [0u8; 32];

        for (i, element) in data.iter_mut().enumerate() {
            // Create AES key from the `element` field element.
            let aes_key = Aes256::new(GenericArray::from_slice(&element.to_bytes_le()));
            let mut tmp = [aes_block[0].clone(), aes_block[1].clone()];
            // Encrypt two blocks at a time.
            aes_key.encrypt_blocks(&mut tmp);

            // Update the `data` with the new field element created from the encrypted bytes.
            *element = FE::from_bytes_le(&[
                tmp[0][0], tmp[0][1], tmp[0][2], tmp[0][3], tmp[0][4], tmp[0][5], tmp[0][6], tmp[0][7],
                tmp[0][8], tmp[0][9], tmp[0][10], tmp[0][11], tmp[0][12], tmp[0][13], tmp[0][14], tmp[0][15],
                tmp[1][0], tmp[1][1], tmp[1][2], tmp[1][3], tmp[1][4], tmp[1][5], tmp[1][6], tmp[1][7],
                tmp[1][8], tmp[1][9], tmp[1][10], tmp[1][11], tmp[1][12], tmp[1][13], tmp[1][14], tmp[1][15]
                ]).expect("Failed to convert random FE.");
        }
    }
}


const NUM_ROUNDS: usize = 4;

// 4 rounds Luby-Rackoff construction
pub struct LubyRackoffPRP {
    keys: Vec<[u8; 16]>,
}

impl LubyRackoffPRP {
    pub fn new(seeds: Option<&[[u8; 16]]>) -> Self {
        let mut keys = vec![[0u8; 16]; NUM_ROUNDS];
        if let Some(s) = seeds {
            keys.copy_from_slice(s);
        } else {
            let mut rng = rand::thread_rng();
            for key in keys.iter_mut() {
                rng.fill(key);
            }
        }

        LubyRackoffPRP {
            keys: keys,
        }
    }

    pub fn permute_block(&self, data: &mut [FE], nblocks: usize) {
        let data_bytes: Vec<[u8; 32]> = data
            .iter()
            .map(|x| x.to_bytes_le())
            .collect();
        let mut data_left: Vec<_> = data_bytes
            .iter()
            .map(|x| {
                let mut block = [0u8; 16];
                block.copy_from_slice(&x[0..16]);
                GenericArray::clone_from_slice(&block)
            })
            .collect();
        let mut data_right : Vec<_> = data_bytes
            .iter()
            .map(|x| {
                let mut block = [0u8; 16];
                block.copy_from_slice(&x[16..32]);
                GenericArray::clone_from_slice(&block)
            })
            .collect();
        let mut tmp: Vec<_> = data_bytes
            .iter()
            .map(|x| {
                let mut block = [0u8; 16];
                block.copy_from_slice(&x[16..32]);
                GenericArray::clone_from_slice(&block)
            })
            .collect();
        
        for i in 0..NUM_ROUNDS {
            tmp.copy_from_slice(&data_left);
            data_left.copy_from_slice(&data_right);
            let aes = Aes128::new(GenericArray::from_slice(&self.keys[i]));
            aes.encrypt_blocks(&mut data_right);
            for j in 0..data.len() {
                data_right[j] = xor_generic_arrays(&data_right[j], &tmp[j]);
            }
        }

        for (i, element) in data.iter_mut().enumerate() {
            let mut block = [0u8; 32];
            block[0..16].copy_from_slice(&data_left[i]);
            block[16..32].copy_from_slice(&data_right[i]);
            *element = FE::from_bytes_le(&block).expect("cannot transform prp-ed block into FE");
        }
    }
}

pub fn xor_block_array(a: &mut [[u8; 16]], b: &[[u8; 16]]) {
    for (block, other_block) in a.iter_mut().zip(b.iter()) {
        // Unroll the loop manually for 16 bytes
        block[0] ^= other_block[0];
        block[1] ^= other_block[1];
        block[2] ^= other_block[2];
        block[3] ^= other_block[3];
        block[4] ^= other_block[4];
        block[5] ^= other_block[5];
        block[6] ^= other_block[6];
        block[7] ^= other_block[7];
        block[8] ^= other_block[8];
        block[9] ^= other_block[9];
        block[10] ^= other_block[10];
        block[11] ^= other_block[11];
        block[12] ^= other_block[12];
        block[13] ^= other_block[13];
        block[14] ^= other_block[14];
        block[15] ^= other_block[15];
    }
}


fn xor_generic_arrays(
    arr1: &GenericArray<u8, U16>,
    arr2: &GenericArray<u8, U16>,
) -> GenericArray<u8, U16> {
    assert_eq!(arr1.len(), arr2.len(), "Arrays must have the same length");

    // Create a new GenericArray by XORing each element
    arr1.iter()
        .zip(arr2.iter())
        .map(|(&a, &b)| a ^ b)
        .collect::<GenericArray<u8, U16>>()
}
