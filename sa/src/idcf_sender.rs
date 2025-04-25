use aes::Aes128;
use aes::cipher::{KeyInit, BlockEncrypt, generic_array::GenericArray};
use psi_network::comm_channel::CommunicationChannel;
use psi_ot::pre_ot::OTPre;
use psi_aes::prg::PRG;
use std::convert::TryInto;
use std::f32::consts::E;
use std::time::Instant;

const NUM_BYTES: usize = 16;
const OT_NUM_BYTES: usize = NUM_BYTES * 3;

// Implementation of Incremental Distributed Comparison Function 
// All values are taken in GF128

pub struct IDCFSender {
    beta: Vec<[u8; NUM_BYTES]>,
    base_ggm_tree: Vec<Vec<[u8; NUM_BYTES]>>,
    implementation_values: Vec<Vec<[u8; NUM_BYTES]>>,
    depth: usize,
    m0: Vec<Vec<[u8; OT_NUM_BYTES]>>,
    m1: Vec<Vec<[u8; OT_NUM_BYTES]>>,
    times: usize,
}

impl IDCFSender {
    pub fn new(depth: usize, times: usize) -> Self {
        let ggm_tree_size = 1 << (depth + 1);
        let mut prg = PRG::new(None, 0);
        let mut seed = [[0u8; 16]; 1];
        prg.random_16byte_block(&mut seed);
        Self {
            beta: vec![[0u8; NUM_BYTES]; times],
            base_ggm_tree: vec![vec![[0u8; NUM_BYTES]; ggm_tree_size]; times],
            implementation_values: vec![vec![[0u8; NUM_BYTES]; ggm_tree_size]; times],
            depth: depth,
            m0: vec![vec![[0u8; 48]; depth + 1]; times],
            m1: vec![vec![[0u8; 48]; depth + 1]; times],
            times: times,
        }
    }

    pub fn compute(&mut self, idcf_sharing: &mut [[u8; NUM_BYTES]], key: [u8; NUM_BYTES], beta: [u8; NUM_BYTES], time: usize) {
        self.beta[time] = beta.clone();
        self.idcf_gen(idcf_sharing, key, time);
    }

    /// Send OT messages and secret sum.
    pub fn send<IO: CommunicationChannel>(&self, io: &mut IO, ot: &mut OTPre<3>, comm: &mut u64) {
        ot.choices_sender(io, comm);
        let mut ot_msg_0 = vec![[0u128; 3]; (self.depth + 1) * self.times];
        for time in 0..self.times {
            for h in 0..self.depth + 1 {
                ot_msg_0[time * (self.depth + 1) + h] = convert_u8_to_u128(&self.m0[time][h]);
            }
        }
        let mut ot_msg_1 = vec![[0u128; 3]; (self.depth + 1) * self.times];
        for time in 0..self.times {
            for h in 0..self.depth + 1 {
                ot_msg_1[time * (self.depth + 1) + h] = convert_u8_to_u128(&self.m1[time][h]);
            }
        }

        ot.send(io, &ot_msg_0, &ot_msg_1, (self.depth + 1) * self.times, 0, comm);

        // for h in 0..(self.depth + 1) {
        //     println!("This OT:");
        //     println!("Sender sent sum of base values for alpha = 0: {:?}", ot_msg_0[h]);
        //     println!("Sender sent sum of base values for alpha = 1: {:?}", ot_msg_1[h]);
        // }
    }

    pub fn idcf_gen(&mut self, idcf_sharing: &mut [[u8; NUM_BYTES]], key: [u8; NUM_BYTES], time: usize) {
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

        // The root of the base GGM tree is the secret (seed)
        self.base_ggm_tree[time][0] = key.clone();

        // Every level after, expand 1-to-2
        for h in 1..self.depth + 1 {
            // Assign base GGM tree values and implementation tree values
            let mut left_blocks: Vec<_> = self.base_ggm_tree[time][((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            // println!("Length of left_blocks: {}", left_blocks.len());
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

            // Compute the left-right sums
            let mut left_base = [0u8; NUM_BYTES];
            let mut right_base = [0u8; NUM_BYTES];
            self.base_ggm_tree[time][((1 << h) - 1)..((1 << (h+1)) - 1)].iter().enumerate().for_each(|(i, x)| {
                if i & 1 == 1 {
                    xor_block(&mut right_base, x);
                } else {
                    xor_block(&mut left_base, x);
                }
            });
            let mut left_impl = [0u8; NUM_BYTES];
            let mut right_impl = [0u8; NUM_BYTES];
            self.implementation_values[time][((1 << h) - 1)..((1 << (h+1)) - 1)].iter().enumerate().for_each(|(i, x)| {
                if i & 1 == 1 {
                    xor_block(&mut right_impl, x);
                } else {
                    xor_block(&mut left_impl, x);
                }
            });

            // println!("Left impl: {:?}", left_impl);
            // println!("Right impl: {:?}", right_impl);
            // println!("Current layer implementation values:");
            // for i in 0..(1 << h) {
            //     println!("{:?}", self.implementation_values[time][(1 << h) - 1 + i]);
            // }

            // Compute the OT messages
            self.m0[time][h][0..16].copy_from_slice(&right_base);
            self.m1[time][h][0..16].copy_from_slice(&left_base);
            if h == 1 {
                xor_block(&mut left_impl, &self.beta[time]);
                xor_block(&mut right_impl, &self.beta[time]);
                self.m0[time][h][16..32].copy_from_slice(&left_impl);
                self.m0[time][h][32..48].copy_from_slice(&right_impl);
                xor_block(&mut left_impl, &self.beta[time]);
                self.m1[time][h][16..32].copy_from_slice(&left_impl);
                self.m1[time][h][32..48].copy_from_slice(&right_impl);
            } else {
                self.m0[time][h][16..32].copy_from_slice(&left_impl);
                self.m0[time][h][32..48].copy_from_slice(&right_impl);
                xor_block(&mut left_impl, &self.beta[time]);
                self.m1[time][h][16..32].copy_from_slice(&left_impl);
                self.m1[time][h][32..48].copy_from_slice(&right_impl);
            }
        }

        idcf_sharing[1] = self.implementation_values[time][1];
        idcf_sharing[2] = self.implementation_values[time][2];
        for h in 2..self.depth + 1 {
            for x in 0..(1 << h) {
                idcf_sharing[(1 << h) - 1 + x as usize] = idcf_sharing[(1 << (h - 1)) - 1 + (x >> 1) as usize];
                xor_block(&mut idcf_sharing[(1 << h) - 1 + x as usize], &self.implementation_values[time][(1 << h) - 1 + x as usize]);
            }
        }
        // println!("IDCF generation time: {:?}", start.elapsed());
    }

    // Only for debug
    pub fn consistency_check<IO: CommunicationChannel>(&self, io: &mut IO, idcf_sharing: &[[u8; NUM_BYTES]], time: usize) {
        io.send_u8(&self.beta[time]).expect("Failed to send beta for testing");
        io.send_block::<NUM_BYTES>(idcf_sharing).unwrap();
    }
}

fn convert_u8_to_u128(a: &[u8; OT_NUM_BYTES]) -> [u128; 3] {
    let mut result = [0u128; 3];
    for i in 0..3 {
        result[i] = u128::from_le_bytes(a[i * NUM_BYTES..(i + 1) * NUM_BYTES].try_into().unwrap());
    }
    result
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