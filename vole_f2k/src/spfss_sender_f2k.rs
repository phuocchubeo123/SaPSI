use std::vec;

use psi_aes::prg::PRG;
use psi_aes::two_key_prp::TwoKeyPRPF2k;
use psi_utils::gf128::{gf128mul, uni_hash_coeff_gen_f2k, vector_inner_product_f2k};
use psi_network::comm_channel::CommunicationChannel;
use psi_ot::pre_ot::OTPre;
use blake3;

pub struct SpfssSenderF2k {
    seed: u128,
    delta: u128,
    secret_sum: u128,
    ggm_tree: Vec<u128>,
    m0: Vec<u128>,
    m1: Vec<u128>,
    depth: usize,
    leave_n: usize,
    prg: PRG,
}

impl SpfssSenderF2k {
    pub fn new(depth: usize) -> Self {
        let leave_n = 1 << (depth - 1);
        let mut prg = PRG::new(None, 0);
        let mut seed_u8 = [[0u8; 16]];
        prg.random_16byte_block(&mut seed_u8);
        let seed = u128::from_le_bytes(seed_u8[0]);
        Self {
            seed,
            delta: 0,
            secret_sum: 0,
            ggm_tree: vec![0; leave_n],
            m0: vec![0; leave_n],
            m1: vec![0; leave_n],
            depth,
            leave_n,
            prg,
        }
    }

    pub fn compute(&mut self, ggm_tree_mem: &mut [u128], secret: u128, gamma: u128) {
        self.delta = secret;
        self.ggm_tree_gen(ggm_tree_mem, secret, gamma);
    }

    pub fn send<IO: CommunicationChannel>(&mut self, io: &mut IO, ot: &mut OTPre<1>, s: usize, comm: &mut u64) {
        let ot_msg_0: Vec<[u128; 1]> = self.m0
            .iter()
            .map(|&x| [x; 1])
            .collect();
        let ot_msg_1: Vec<[u128; 1]> = self.m1
            .iter()
            .map(|&x| [x; 1])
            .collect();
        ot.send(io, &ot_msg_0, &ot_msg_1, self.depth - 1, s, comm);
        *comm += io.send_block::<16>(&[self.secret_sum.to_le_bytes()]).expect("Failed to send secret sum");
    }

    fn ggm_tree_gen(&mut self, ggm_tree_mem: &mut [u128], secret: u128, gamma: u128) {
        let mut prp = TwoKeyPRPF2k::new([[0u8; 16], [1u8; 16]]);
        prp.expand_left(&[self.seed], &mut ggm_tree_mem[0..1]);
        prp.expand_right(&[self.seed], &mut ggm_tree_mem[1..2]);
        self.m0[0] = ggm_tree_mem[0];
        self.m1[0] = ggm_tree_mem[1];

        for h in 1..self.depth - 1 {
            self.m0[h] = 0;
            self.m1[h] = 0;
            let sz = 1 << h;
            prp.expand_left(&ggm_tree_mem[..sz], &mut self.ggm_tree[..sz]);
            prp.expand_right(&ggm_tree_mem[..sz], &mut self.ggm_tree[sz..2 * sz]);
            for i in (0..sz).rev() {
                ggm_tree_mem[2*i] = self.ggm_tree[i];
                ggm_tree_mem[2*i+1] = self.ggm_tree[i+sz];
                self.m0[h] ^= ggm_tree_mem[2*i];
                self.m1[h] ^= ggm_tree_mem[2*i+1];
            }
        }

        self.secret_sum = 0;
        for i in 0..self.leave_n {
            self.secret_sum ^= ggm_tree_mem[i];
        }
        self.secret_sum ^= gamma;
        self.ggm_tree.copy_from_slice(&ggm_tree_mem[..self.leave_n]);
    }

    pub fn consistency_check<IO: CommunicationChannel>(&self, io: &mut IO, y: u128, comm: &mut u64) {
        // z = y + delta * beta

        let hash = blake3::hash(&self.secret_sum.to_le_bytes());
        let mut hsh = [0u8; 16];
        hsh.copy_from_slice(&hash.as_bytes()[..16]);
        let uni_hash_seed = u128::from_le_bytes(hsh);
        let mut chi = vec![0u128; self.leave_n];
        uni_hash_coeff_gen_f2k(&mut chi, uni_hash_seed, self.leave_n);

        let x_star_bytes = io.receive_block::<16>().expect("Failed to receive x_star");
        let x_star = u128::from_le_bytes(x_star_bytes[0]);
        let y_star = y ^ gf128mul(self.delta, x_star);
        let v = vector_inner_product_f2k(&chi, &self.ggm_tree) ^ y_star;
        
        *comm += io.send_block::<16>(&[v.to_le_bytes()]).expect("Failed to send V");
    }

    pub fn consistency_check_msg_gen(&self, v: &mut u128, seed: u128) {
        let mut chi = vec![0u128; self.leave_n];
        let hash = blake3::hash(&seed.to_le_bytes());
        let mut hsh = [0u8; 16];
        hsh.copy_from_slice(&hash.as_bytes()[..16]);
        let uni_hash_seed = u128::from_le_bytes(hsh);
        uni_hash_coeff_gen_f2k(&mut chi, uni_hash_seed, self.leave_n);
        *v = vector_inner_product_f2k(&chi, &self.ggm_tree);
    }
}

