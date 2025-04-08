use aes::Aes128;
use aes::cipher::generic_array::GenericArray;
use psi_network::comm_channel::CommunicationChannel;
use psi_ot::pre_ot::OTPre;
use psi_aes::prg::PRG;

const NUM_BYTES: usize = 16;

// Implementation of Incremental Distributed Comparison Function 
// All values are taken in GF128

pub struct IDCFSender {
    seed: [u8; NUM_BYTES],
    delta: [u8; NUM_BYTES],
    secret_sum: [u8; NUM_BYTES],
    base_ggm_tree: Vec<[u8; NUM_BYTES]>,
    implementation_values: Vec<[u8; NUM_BYTES]>,
    depth: usize,
    m0: Vec<[u8; 48]>,
    m1: Vec<[u8; 48]>,
}

impl IDCFSender {
    pub fn new(depth: usize) -> Self {
        let ggm_tree_size = 1 << depth;
        let mut prg = PRG::new(None, 0);
        let mut seed = [[0u8; 16]; 1];
        prg.random_16byte_block(&mut seed);
        Self {
            seed: seed[0],
            delta: [0u8; NUM_BYTES],
            secret_sum: [0u8; NUM_BYTES],
            base_ggm_tree: vec![[0u8; NUM_BYTES]; ggm_tree_size],
            implementation_values: vec![[0u8; NUM_BYTES]; ggm_tree_size * 2],
            depth: depth,
            m0: vec![[0u8; 48]; depth],
            m1: vec![[0u8; 48]; depth],
        }
    }

    pub fn compute(&mut self, base_ggm_tree_mem: &mut [[u8; NUM_BYTES]], implementation_tree_mem: &mut [[u8; NUM_BYTES]], secret: [u8; NUM_BYTES], gamma: [u8; NUM_BYTES]) {
        self.delta = secret.clone();
        self.idcf_gen(base_ggm_tree_mem, implementation_tree_mem, secret, gamma);
    }

    /// Send OT messages and secret sum.
    pub fn send<IO: CommunicationChannel>(&self, io: &mut IO, ot: &mut OTPre<3>, s: usize, comm: &mut u64) {
        let ot_msg_0 = self.m0
            .iter()
            .map(|x| x.to_le_bytes())
            .collect::<Vec<[u8; NUM_BYTES]>>();
        let ot_msg_1 = self.m1
            .iter()
            .map(|x| x.to_bytes_le())
            .collect::<Vec<[u8; NUM_BYTES]>>();

        ot.send(io, &ot_msg_0, &ot_msg_1, self.depth - 1, s, comm);
        *comm += io.send_block::<NUM_BYTES>(&[self.secret_sum]).expect("Failed to send secret sum.");
    }

    pub fn idcf_gen(&mut self, base_ggm_tree_mem: &mut [[u8; NUM_BYTES]], implementation_tree_mem: &mut [[u8; NUM_BYTES]], secret: [u8; NUM_BYTES], gamma: [u8; NUM_BYTES]) {
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
        base_ggm_tree_mem[0] = secret.clone();

        // Every level after, expand 1-to-2
        for h in 1..self.depth {
            let left_blocks: Vec<_> = base_ggm_tree_mem[((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();
            let right_blocks: Vec<_> = base_ggm_tree_mem[((1 << (h-1)) - 1)..((1 << h) - 1)]
                .iter()
                .map(|x| GenericArray::clone_from_slice(x))
                .collect();

            // Assign base GGM tree values and implementation tree values
            g0.encrypt_blocks(&mut left_blocks);
            base_ggm_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].copy_from_slice(&left_blocks);
            g1.encrypt_blocks(&mut right_blocks);
            base_ggm_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].copy_from_slice(&right_blocks);
            c0.encrypt_blocks(&mut left_blocks);
            implementation_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].copy_from_slice(&left_blocks);
            c1.encrypt_blocks(&mut right_blocks);
            implementation_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].copy_from_slice(&right_blocks);

            // Compute the left-right sums
            let mut left_base = [0u8; NUM_BYTES];
            base_ggm_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].iter().for_each(|x| xor_block(&mut left_base, x));
            let mut right_base = [0u8; NUM_BYTES];
            base_ggm_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].iter().for_each(|x| xor_block(&mut right_base, x));
            let mut left_impl = [0u8; NUM_BYTES];
            implementation_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].iter().for_each(|x| xor_block(&mut left_impl, x));
            let mut right_impl = [0u8; NUM_BYTES];
            implementation_tree_mem[((1 << h) - 1)..((1 << (h+1)) - 1)].iter().for_each(|x| xor_block(&mut right_impl, x));

            // Compute the OT messages
            self.m0[h][0..16].copy_from_slice(&right_base);
            self.m1[h][0..16].copy_from_slice(&left_base);
            if h == 1 {
                xor_block(&mut left_impl, &self.delta);
                xor_block(&mut right_impl, &self.delta);
                self.m0[h][16..32].copy_from_slice(&left_impl);
                self.m0[h][32..48].copy_from_slice(&right_impl);
                xor_block(&mut left_impl, &self.delta);
                self.m1[h][16..32].copy_from_slice(&left_impl);
                self.m1[h][32..48].copy_from_slice(&right_impl);
            } else {
                self.m0[h][16..32].copy_from_slice(&left_impl);
                self.m0[h][32..48].copy_from_slice(&right_impl);
                xor_block(&mut left_impl, &self.delta);
                self.m1[h][16..32].copy_from_slice(&left_impl);
                self.m1[h][32..48].copy_from_slice(&right_impl);
            }
        }
    }
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