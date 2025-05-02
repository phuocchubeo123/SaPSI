use aes::Aes128;
use aes::cipher::{KeyInit, BlockEncrypt, generic_array::GenericArray};
use psi_network::comm_channel::CommunicationChannel;
use psi_ot::pre_ot::OTPre;

const NUM_BYTES: usize = 16;
const OT_NUM_BYTES: usize = NUM_BYTES * 3;

pub struct IDCFReceiver {
    depth: usize,
    alpha: Vec<Vec<bool>>,
    base_ggm_tree: Vec<Vec<[u8; NUM_BYTES]>>,
    implementation_values: Vec<Vec<[u8; NUM_BYTES]>>,
    m: Vec<Vec<[u8; OT_NUM_BYTES]>>,
    times: usize,
}

impl IDCFReceiver {
    pub fn new(depth: usize, times: usize) -> Self {
        let ggm_tree_size = 1 << (depth + 1);
        IDCFReceiver {
            depth,
            alpha: vec![vec![false; depth + 1]; times],
            base_ggm_tree: vec![vec![[0u8; 16]; ggm_tree_size]; times],
            implementation_values: vec![vec![[0u8; 16]; ggm_tree_size]; times],
            m: vec![vec![[0u8; 48]; depth + 1]; times],
            times: times,
        }
    }

    pub fn set_alpha(&mut self, alpha: [u8; 16], time: usize) {
        // We always assume that alpha here will not have more than 128 bits
        for i in 0..self.depth {
            self.alpha[time][self.depth - i] = ((alpha[i / 8] >> (i % 8)) & 1) == 1;
        }
    }

    // Here, alpha only has depth number of bits
    pub fn receive<IO: CommunicationChannel>(&mut self, io: &mut IO, ot: &mut OTPre<3>, comm: &mut u64) {
        // println!("Alpha bits: {:?}", &self.alpha[0..self.depth + 1]);
        let mut choices = vec![false; (self.depth + 1) * self.times];
        for time in 0..self.times {
            for h in 0..self.depth + 1 {
                choices[time * (self.depth + 1) + h] = self.alpha[time][h];
            }
        }

        ot.choices_recver(io, &choices, comm);
        ot.reset();

        let mut ot_msg = vec![[0u128; 3]; (self.depth + 1) * self.times];
        ot.recv(io, &mut ot_msg, &choices, (self.depth + 1) * self.times, 0, comm);

        // for time in 0..self.times {
        //     for h in 0..self.depth + 1 {
        //         println!("OT number: {}", time * (self.depth + 1) + h);
        //         println!("OT choice: {}", choices[time * (self.depth + 1) + h]);
        //         println!("OT message: {:?}", ot_msg[time * (self.depth + 1) + h]);
        //     }
        // }

        for time in 0..self.times {
            for h in 0..self.depth + 1 {
                self.m[time][h] = convert_u128_to_u8(&ot_msg[time * (self.depth + 1) + h]);
            }
        }
    }

    pub fn compute(&mut self, idcf_sharing: &mut [[u8; 16]], time: usize) {
        self.idcf_reconstruct(idcf_sharing, time);
    }

    pub fn idcf_reconstruct(&mut self, idcf_sharing: &mut [[u8; 16]], time: usize) {
        // Here, we assume fixed key AES to be Random Oracle
        let mut kg0 = [0u8; 16];
        let mut kg1 = [0u8; 16];
        let mut kc0 = [0u8; 16];
        let mut kc1 = [0u8; 16];
        kg0[0] = 0u8;
        kg1[0] = 1u8;
        kc0[0] = 2u8;
        kc1[0] = 3u8;
        let mut g0 = Aes128::new(GenericArray::from_slice(&kg0));
        let mut g1 = Aes128::new(GenericArray::from_slice(&kg1));
        let mut c0 = Aes128::new(GenericArray::from_slice(&kc0));
        let mut c1 = Aes128::new(GenericArray::from_slice(&kc1));

        let mut missing_pos: usize = 0;
        let mut fill_pos: usize = 0;
        for h in 1..self.depth + 1 {
            // Fill a_1 ... \bar{a_h} and fill next layer nodes
            missing_pos = (missing_pos << 1) | (self.alpha[time][h] as usize);
            fill_pos = missing_pos ^ 1;
            // println!("Missing position: {}", missing_pos);
            // println!("Fill position: {}", fill_pos);
            // Assign base GGM tree values and implementation tree values for the non-missing layer nodes
            let mut left_blocks: Vec<_> = self.base_ggm_tree[time][((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            g0.encrypt_blocks(&mut left_blocks);
            for i in 0..(1 << (h - 1)) {
                self.base_ggm_tree[time][((1 << h) - 1) + (i << 1)].copy_from_slice(&left_blocks[i]);
            }

            let mut right_blocks: Vec<_> = self.base_ggm_tree[time][((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            g1.encrypt_blocks(&mut right_blocks);
            for i in 0..(1 << (h - 1)) {
                self.base_ggm_tree[time][((1 << h) - 1) + ((i << 1) ^ 1)].copy_from_slice(&right_blocks[i]);
            }

            let mut left_blocks: Vec<_> = self.base_ggm_tree[time][((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            c0.encrypt_blocks(&mut left_blocks);
            for i in 0..(1 << (h - 1)) {
                self.implementation_values[time][((1 << h) - 1) + (i << 1)].copy_from_slice(&left_blocks[i]);
            }

            let mut right_blocks: Vec<_> = self.base_ggm_tree[time][((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            c1.encrypt_blocks(&mut right_blocks);
            for i in 0..(1 << (h - 1)) {
                self.implementation_values[time][((1 << h) - 1) + ((i << 1) ^ 1)].copy_from_slice(&right_blocks[i]);
            }

            // println!("Current layer implementation values:");
            // for i in 0..(1 << h) {
            //     println!("{:?}", self.implementation_values[(1 << h) - 1 + i]);
            // }


            // Fill in the non-critical-path hole in this layer
            self.base_ggm_tree[time][(1 << h) - 1 + fill_pos].copy_from_slice(&self.m[time][h][0..16]);
            for i in 0..(1 << (h - 1)) {
                let pos: usize = (1 << h) - 1 + (((i << 1) | (self.alpha[time][h] as usize)) ^ 1);
                let val: [u8; 16] = self.base_ggm_tree[time][pos];
                if  pos != (1 << h) - 1 + fill_pos {
                    xor_block(&mut self.base_ggm_tree[time][(1 << h) - 1 + fill_pos], &val);
                }
            }

            // Now fill in the hole in the implementation tree
            self.implementation_values[time][(1 << h) - 1 + ((missing_pos | 1) ^ 1)].copy_from_slice(&self.m[time][h][16..32]);
            for i in 0..(1 << (h - 1)) {
                let pos: usize = (1 << h) - 1 + (i << 1);
                let val: [u8; 16] = self.implementation_values[time][pos];
                if pos != (1 << h) - 1 + ((missing_pos | 1) ^ 1) {
                    xor_block(&mut self.implementation_values[time][(1 << h) - 1 + ((missing_pos | 1) ^ 1)], &val);
                }
            }

            self.implementation_values[time][(1 << h) - 1 + (missing_pos | 1)].copy_from_slice(&self.m[time][h][32..48]);
            for i in 0..(1 << (h - 1)) {
                let pos: usize = (1 << h) - 1 + ((i << 1) ^ 1);
                let val: [u8; 16] = self.implementation_values[time][pos];
                if pos != (1 << h) - 1 + (missing_pos | 1) {
                    xor_block(&mut self.implementation_values[time][(1 << h) - 1 + (missing_pos | 1)], &val);
                }
            }

            // println!("Current layer implementation values after filling:");
            // for i in 0..(1 << h) {
            //     println!("{:?}", self.implementation_values[time][(1 << h) - 1 + i]);
            // }


        }

        idcf_sharing[1] = self.implementation_values[time][1];
        idcf_sharing[2] = self.implementation_values[time][2];
        for h in 2..self.depth + 1 {
            for x in 0..(1 << h) {
                idcf_sharing[(1 << h) - 1 + x as usize] = idcf_sharing[(1 << (h - 1)) - 1 + (x >> 1) as usize];
                xor_block(&mut idcf_sharing[(1 << h) - 1 + x as usize], &self.implementation_values[time][(1 << h) - 1 + x as usize]);
            }
        }
    }

    pub fn consistency_check<IO: CommunicationChannel>(&self, io: & mut IO, idcf_sharing: &[[u8; NUM_BYTES]], time: usize) {
        let beta = io.receive_u8().expect("Failed to receive beta in test");
        let sender_idcf_sharing = io.receive_block::<NUM_BYTES>().expect("Failed to receive IDCF sharing in test");
        // Check the consistency of the base GGM tree and implementation tree
        let mut alpha_pref: u128 = 0;
        for h in 1..(self.depth + 1) {
            alpha_pref = (alpha_pref << 1) + (self.alpha[time][h] as u128);
            for x in 0..(1 << h) {
                if x < alpha_pref {
                    let mut shared_value = idcf_sharing[(1 << h) - 1 + x as usize];
                    xor_block(&mut shared_value, &sender_idcf_sharing[(1 << h) - 1 + x as usize]);
                    assert_eq!(shared_value.to_vec(), vec![0u8; 16], "IDCF sharing mismatch at depth {} and index {}", h, x);
                } else {
                    let mut shared_value = idcf_sharing[(1 << h) - 1 + x as usize];
                    xor_block(&mut shared_value, &sender_idcf_sharing[(1 << h) - 1 + x as usize]);
                    assert_eq!(shared_value.to_vec(), beta, "IDCF sharing mismatch at depth {} and index {}", h, x);
                }
            }
        }
        println!("IDCF sharing consistency check passed");
    }
}

fn convert_u128_to_u8(input: &[u128; 3]) -> [u8; OT_NUM_BYTES] {
    let mut output = [0u8; OT_NUM_BYTES];
    for i in 0..3 {
        output[i * 16..(i + 1) * 16].copy_from_slice(&input[i].to_le_bytes());
    }
    output
}

fn xor_block(a: &mut [u8; NUM_BYTES], b: &[u8; NUM_BYTES]) {
    // Unroll for 16 bytes
    a[0] ^= b[0];
    a[1] ^= b[1];
    a[2] ^= b[2];
    a[3] ^= b[3];
    a[4] ^= b[4];
    a[5] ^= b[5];
    a[6] ^= b[6];
    a[7] ^= b[7];
    a[8] ^= b[8];
    a[9] ^= b[9];
    a[10] ^= b[10];
    a[11] ^= b[11];
    a[12] ^= b[12];
    a[13] ^= b[13];
    a[14] ^= b[14];
    a[15] ^= b[15];
}