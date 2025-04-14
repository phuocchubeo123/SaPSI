use aes::Aes128;
use aes::cipher::{KeyInit, BlockEncrypt, generic_array::GenericArray};
use psi_network::comm_channel::CommunicationChannel;
use psi_ot::pre_ot::OTPre;

const NUM_BYTES: usize = 16;
const OT_NUM_BYTES: usize = NUM_BYTES * 3;

pub struct IDCFReceiver {
    depth: usize,
    alpha: Vec<bool>,
    base_ggm_tree: Vec<[u8; NUM_BYTES]>,
    implementation_tree: Vec<[u8; NUM_BYTES]>,
    m: Vec<[u8; OT_NUM_BYTES]>,
}

impl IDCFReceiver {
    pub fn new(depth: usize, alpha: &[bool]) -> Self {
        assert_eq!(alpha.len(), depth);
        let ggm_tree_size = (1 << (depth + 1)) - 1;
        IDCFReceiver {
            depth,
            alpha: alpha.to_vec(),
            base_ggm_tree: vec![[0u8; 16]; ggm_tree_size],
            implementation_tree: vec![[0u8; 16]; ggm_tree_size],
            m: vec![[0u8; 48]; depth],
        }
    }

    // Here, alpha only has depth number of bits
    pub fn receive<IO: CommunicationChannel>(&mut self, io: &mut IO, ot: &mut OTPre<3>, s: usize, comm: &mut u64) {
        let mut ot_msg = vec![[0u128; 3]; self.depth];
        ot.recv(io, &mut ot_msg, &self.alpha, self.depth, s, comm);
        for h in 0..self.depth {
            self.m[h] = convert_u128_to_u8(&ot_msg[h]);
        }
    }

    pub fn compute(&mut self, base_ggm_tree_mem: &mut [[u8; 16]], implementation_tree_mem: &mut [[u8; 16]], alpha: [u8; 16]) {
        // We always assume that alpha here will not have more than 128 bits
        for i in 0..self.depth {
            self.alpha[i] = ((alpha[i / 8] >> (i % 8)) & 1) == 1;
        }
        self.idcf_reconstruct(base_ggm_tree_mem, implementation_tree_mem);
    }

    pub fn idcf_reconstruct(&mut self, base_ggm_tree_mem: &mut[[u8; 16]], implementation_tree_mem: &mut [[u8; 16]]) {
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
        for h in 1..self.depth {
            // Fill a_1 ... \bar{a_h} and fill next layer nodes
            missing_pos = (missing_pos << 1) | (self.alpha[h] as usize);
            fill_pos = missing_pos ^ 1;
            self.base_ggm_tree[(1 << h) - 1 + fill_pos].copy_from_slice(&self.m[h][0..16]);
            for i in 0..(1 << (h - 1)) {
                let pos: usize = (1 << h) - 1 + (((i << 1) | (self.alpha[h] as usize)) ^ 1);
                let val: [u8; 16] = self.base_ggm_tree[pos];
                if  pos != (1 << h) - 1 + fill_pos {
                    xor_block(&mut self.base_ggm_tree[fill_pos], &val);
                }
            }

            // Assign base GGM tree values and implementation tree values for the non-missing next layer nodes
            let mut left_blocks: Vec<_> = base_ggm_tree_mem[((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            g0.encrypt_blocks(&mut left_blocks);
            for i in 0..(1 << h) {
                base_ggm_tree_mem[((1 << h) - 1) + i].copy_from_slice(&left_blocks[i]);
            }

            let mut right_blocks: Vec<_> = base_ggm_tree_mem[((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            g1.encrypt_blocks(&mut right_blocks);
            for i in 0..(1 << h) {
                base_ggm_tree_mem[((1 << h) - 1) + i].copy_from_slice(&right_blocks[i]);
            }

            let mut left_blocks: Vec<_> = base_ggm_tree_mem[((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            c0.encrypt_blocks(&mut left_blocks);
            for i in 0..(1 << h) {
                implementation_tree_mem[((1 << h) - 1) + i].copy_from_slice(&left_blocks[i]);
            }

            let mut right_blocks: Vec<_> = base_ggm_tree_mem[((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            c1.encrypt_blocks(&mut right_blocks);
            for i in 0..(1 << h) {
                implementation_tree_mem[((1 << h) - 1) + i].copy_from_slice(&right_blocks[i]);
            }

            // Now fill in the hole in the implementation tree
            self.implementation_tree[(1 << h) - 1 + (missing_pos & 0)].copy_from_slice(&self.m[h][16..32]);
            for i in 0..(1 << (h - 1)) {
                let pos: usize = (1 << h) - 1 + (i << 1);
                let val: [u8; 16] = self.implementation_tree[pos];
                if pos != (1 << h) - 1 + (missing_pos & 0) {
                    xor_block(&mut self.implementation_tree[missing_pos], &val);
                }
            }

            self.implementation_tree[(1 << h) - 1 + (missing_pos & 1)].copy_from_slice(&self.m[h][32..48]);
            for i in 0..(1 << (h - 1)) {
                let pos: usize = (1 << h) - 1 + ((i << 1) ^ 1);
                let val: [u8; 16] = self.implementation_tree[pos];
                if pos != (1 << h) - 1 + (missing_pos & 1) {
                    xor_block(&mut self.implementation_tree[missing_pos], &val);
                }
            }
        }

        base_ggm_tree_mem.copy_from_slice(&self.base_ggm_tree);
        implementation_tree_mem.copy_from_slice(&self.implementation_tree);
    }

    pub fn consistency_check<IO: CommunicationChannel>(&self, io: & mut IO, idcf_sharing: &[[u8; NUM_BYTES]]) {
        let beta = io.receive_u8().expect("Failed to receive beta in test");
        let sender_idcf_sharing = io.receive_block::<NUM_BYTES>().expect("Failed to receive IDCF sharing in test");
        // Check the consistency of the base GGM tree and implementation tree
        let mut alpha_pref: u128 = 0;
        for h in 1..(self.depth + 1) {
            alpha_pref = (alpha_pref << 1) + (self.alpha[h] as u128);
            for x in 0..(1 << h) {
                if x < alpha_pref {
                    let mut shared_value = idcf_sharing[(1 << h) - 1 + x as usize];
                    xor_block(&mut shared_value, &sender_idcf_sharing[(1 << h) - 1 + x as usize]);
                    assert_eq!(shared_value.to_vec(), beta, "IDCF sharing mismatch at depth {} and index {}", h, x);
                }
            }
        }
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