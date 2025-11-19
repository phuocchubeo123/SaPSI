use psi_network::tcp_channel::TcpChannel;
use psi_ot::pre_ot::OTPre;
use psi_aes::{two_key_prp::TwoKeyPRPF2k};
use blake3;
use psi_utils::gf128::{gf128mul, uni_hash_coeff_gen_f2k, vector_inner_product_f2k};
pub struct SpfssRecverF2k {
    ggm_tree: Vec<u128>,
    m: Vec<u128>,
    pub(crate) b: Vec<bool>,
    choice_pos: usize,
    depth: usize,
    leave_n: usize,
    share: u128,
}

impl SpfssRecverF2k {
    pub fn new(depth: usize) -> Self {
        let leave_n = 1 << (depth - 1);
        Self {
            ggm_tree: vec![0; leave_n],
            m: vec![0; depth - 1],
            b: vec![false; depth - 1],
            choice_pos: (1 << depth - 1) - 1,
            depth,
            leave_n,
            share: 0,
        }
    }

    pub fn get_index(&self) -> usize {
        let mut choice_pos = 0;
        for i in 0..self.depth-1 {
            choice_pos <<= 1;
            if !self.b[i] {
                choice_pos += 1;
            }
        }
        choice_pos
    }

    pub fn recv(&mut self, io: &mut TcpChannel, ot: &mut OTPre<1>, s: usize) {
        let mut receive_data = vec![[0u128; 1]; self.depth - 1];
        ot.recv(io, &mut receive_data, &mut self.b, self.depth - 1, s);

        self.m = receive_data
            .iter()
            .map(|&x| x[0])
            .collect::<Vec<u128>>();
        
        let share_bytes = io.receive_block::<16>().expect("Failed to receive share");
        self.share = u128::from_le_bytes(share_bytes[0]);
    }

    pub fn compute(&mut self, ggm_tree_mem: &mut [u128], delta2: u128) {
        self.reconstruct_tree();
        self.ggm_tree[self.choice_pos] = 0;

        let mut nodes_sum = 0;
        for i in 0..self.leave_n {
            nodes_sum ^= self.ggm_tree[i];
        }

        nodes_sum ^= self.share;
        self.ggm_tree[self.choice_pos] = delta2 ^ nodes_sum;

        ggm_tree_mem.copy_from_slice(&self.ggm_tree);
    }

    fn reconstruct_tree(&mut self) {
        let mut to_fill_idx = 0;
        let mut prp = TwoKeyPRPF2k::new([[0u8; 16], [1u8; 16]]);
        for i in 1..self.depth {
            to_fill_idx *= 2;
            self.ggm_tree[to_fill_idx] = 0;
            self.ggm_tree[to_fill_idx + 1] = 0;

            if !self.b[i - 1] {
                self.layer_recover(i, 0, to_fill_idx, self.m[i - 1], &mut prp);
                to_fill_idx += 1;
            } else {
                self.layer_recover(i, 1, to_fill_idx + 1, self.m[i - 1], &mut prp);
            }
        }
    }

    fn layer_recover(
        &mut self,
        depth: usize,
        lr: usize, 
        to_fill_idx: usize,
        sum: u128,
        prp: &mut TwoKeyPRPF2k,
    ) {
        let item_n = 1 << depth;
        let mut nodes_sum = 0;
        let mut lr_start = 0;
        if lr != 0 {
            lr_start = 1;
        }
        for i in (lr_start..item_n).step_by(2) {
            nodes_sum ^= self.ggm_tree[i];
        }
        self.ggm_tree[to_fill_idx] = nodes_sum ^ sum;
        if depth == self.depth - 1 {
            return;
        }
        let mut tmp = self.ggm_tree.clone();
        prp.expand_left(&self.ggm_tree[..item_n], &mut tmp[..item_n]);
        prp.expand_right(&self.ggm_tree[..item_n], &mut tmp[item_n..2 * item_n]);
        for i in (0..item_n).rev() {
            self.ggm_tree[2 * i] = tmp[i];
            self.ggm_tree[2 * i + 1] = tmp[i + item_n];
        }
    }

    pub fn consistency_check(&self, io: &mut TcpChannel, z: u128, beta: u128) {
        let hash = blake3::hash(&self.share.to_le_bytes());
        let mut hash_bytes = [0u8; 16];
        hash_bytes.copy_from_slice(&hash.as_bytes()[0..16]);
        let uni_hash_seed = u128::from_le_bytes(hash_bytes);
        let mut chi = vec![0u128; self.leave_n];
        uni_hash_coeff_gen_f2k(&mut chi, uni_hash_seed, self.leave_n);
        // Compute and send x_star
        let x_star = gf128mul(chi[self.choice_pos], beta) ^ beta;
        io.send_block::<16>(&[x_star.to_le_bytes()]).expect("Failed to send x_star"); 
        // Compute W 
        let w = vector_inner_product_f2k(&chi, &self.ggm_tree) ^ z;
        // Receive and check V
        let v_bytes = io.receive_block::<16>().expect("Failed to receive V");
        let v = u128::from_le_bytes(v_bytes[0]);
        
        if w != v {
            panic!("Consistency check failed: w != v");
        } else {
            println!("Consistency check successful for SpfssF2k!");
        }
    }

    pub fn consistency_check_msg_gen(&self, chi_alpha: &mut u128, w: &mut u128, seed: u128) {
        let mut chi = vec![0u128; self.leave_n];
        let hash = blake3::hash(&seed.to_le_bytes());
        let mut hash_bytes = [0u8; 16];
        hash_bytes.copy_from_slice(&hash.as_bytes()[0..16]);
        let uni_hash_seed = u128::from_le_bytes(hash_bytes);
        uni_hash_coeff_gen_f2k(&mut chi, uni_hash_seed, self.leave_n);
        *chi_alpha = chi[self.choice_pos];
        *w = vector_inner_product_f2k(&chi, &self.ggm_tree);
    }
}