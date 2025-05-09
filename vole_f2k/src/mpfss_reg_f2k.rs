use crate::spfss_sender_f2k::SpfssSenderF2k;
use crate::spfss_receiver_f2k::SpfssRecverF2k;
use psi_aes::prg::PRG;
use psi_ot::pre_ot::OTPre;
use psi_network::comm_channel::CommunicationChannel;
use psi_utils::gf128::gf128mul;
use blake3;

pub struct MpfssRegF2k {
    party: usize,
    item_n: usize,
    idx_max: usize, 
    m: usize,
    tree_height: usize,
    leave_n: usize,
    tree_n: usize,
    is_malicious: bool,
    prg: PRG,
    secret_share_x: u128,
    ggm_tree: Vec<Vec<u128>>,
    check_chialpha_buf: Vec<u128>,
    check_vw_buf: Vec<u128>,
    item_pos_receiver: Vec<usize>,
    triple_y: Vec<u128>,
    triple_z: Vec<u128>,
}

impl MpfssRegF2k {
    pub fn new(n: usize, t: usize, log_bin_sz: usize, party: usize) -> Self {
        // make sure n = t * leave_n
        MpfssRegF2k {
            party: party,
            item_n: t,
            idx_max: n,
            m: 0,
            tree_height: log_bin_sz + 1,
            leave_n: 1 << log_bin_sz,
            tree_n: t,
            is_malicious: false,
            prg: PRG::new(None, 0),
            secret_share_x: 0,
            ggm_tree: vec![vec![0u128; 1 << log_bin_sz]; t],
            check_chialpha_buf: vec![0u128; t],
            check_vw_buf: vec![0u128; t],
            item_pos_receiver: vec![0; t],
            triple_y: vec![0u128; t+1],
            triple_z: vec![0u128; t+1],
        }
    }

    pub fn set_malicious(&mut self) {
        self.is_malicious = true;
    }

    pub fn sender_init(&mut self, delta: u128) {
        self.secret_share_x = delta.clone();
    }

    pub fn receiver_init(&mut self) {
    }

    pub fn set_vec_x(&self, out_vec: &mut [u128], in_vec: &[u128]) {
        for i in 0..self.tree_n {
            let pt = i * self.leave_n + self.item_pos_receiver[i] % self.leave_n;
            out_vec[pt] += in_vec[i];
        }
    }

    pub fn mpfss_sender<IO: CommunicationChannel>(&mut self, io: &mut IO, ot: &mut OTPre<1>, triple_y: &[u128], sparse_vector: &mut [u128], comm: &mut u64) {
        // triple_y_recv = triple_y_send + delta * triple_z

        self.triple_y.copy_from_slice(&triple_y[..self.tree_n+1]);

        // Set up PreOT first
        for i in 0..self.tree_n {
            ot.choices_sender(io, comm);
        }
        io.flush();
        ot.reset();

        let mut seeds = vec![0u128; self.tree_n];
        if self.is_malicious {
            self.seed_expand(io, &mut seeds, self.tree_n, comm);
        }
        io.flush();

        // Now start doing Spfss
        for i in 0..self.tree_n {
            let mut sender = SpfssSenderF2k::new(self.tree_height);
            sender.compute(&mut self.ggm_tree[i], self.secret_share_x, self.triple_y[i]);
            sender.send(io, ot, i, comm);
            sparse_vector[i*self.leave_n..(i+1)*self.leave_n].copy_from_slice(&self.ggm_tree[i]);

            // Malicious check
            if self.is_malicious {
                sender.consistency_check_msg_gen(&mut self.check_vw_buf[i], seeds[i]);
            }
        }

        // consistency batch check
        if self.is_malicious {
            let x_star_bytes = io.receive_block::<16>().expect("Failed to receive x_star")[0];
            let x_star = u128::from_le_bytes(x_star_bytes);
            // tmp should be equal to triple_y_recv[self.tree_n] - something
            let tmp = gf128mul(self.secret_share_x, x_star) ^ self.triple_y[self.tree_n];
            let mut vb = 0u128;
            vb = vb ^ tmp;
            for i in 0..self.tree_n {
                vb ^= self.check_vw_buf[i];
            }

            let hash = blake3::hash(&vb.to_le_bytes());
            let mut h = [0u8; 32];
            h.copy_from_slice(hash.as_bytes());
            *comm += io.send_block::<32>(&[h]).expect("Failed to send h");
        }
    }

    pub fn mpfss_receiver<IO: CommunicationChannel>(&mut self, io: &mut IO, ot: &mut OTPre<1>, triple_y: &[u128], triple_z: &[u128], sparse_vector_y: &mut [u128], sparse_vector_z: &mut [u128], comm: &mut u64) {
        // triple_y_recv = triple_y_send + delta * triple_z

        self.triple_y.copy_from_slice(&triple_y[..self.tree_n+1]);
        self.triple_z.copy_from_slice(&triple_z[..self.tree_n+1]);

        for i in 0..self.tree_n {
            let b = vec![false; self.tree_height - 1];
            ot.choices_recver(io, &b, comm);
        }
        io.flush();
        ot.reset();

        let mut seeds = vec![0u128; self.tree_n];
        if self.is_malicious {
            self.seed_expand(io, &mut seeds, self.tree_n, comm);
        }

        for i in 0..self.tree_n {
            let mut receiver = SpfssRecverF2k::new(self.tree_height);
            self.item_pos_receiver[i] = receiver.get_index();
            receiver.recv(io, ot, i, comm);
            receiver.compute(&mut self.ggm_tree[i], self.triple_y[i]);
            sparse_vector_y[i*self.leave_n..(i+1)*self.leave_n].copy_from_slice(&self.ggm_tree[i]);
            for j in i*self.leave_n..(i+1)*self.leave_n {
                sparse_vector_z[j] = 0u128;
            }
            sparse_vector_z[i*self.leave_n + self.item_pos_receiver[i]] = self.triple_z[i];

            if self.is_malicious {
                receiver.consistency_check_msg_gen(&mut self.check_chialpha_buf[i], &mut self.check_vw_buf[i], seeds[i]);
            }
        }

        if self.is_malicious {
            let mut beta_mul_chialpha = 0u128;
            for i in 0..self.tree_n {
                beta_mul_chialpha ^= gf128mul(self.check_chialpha_buf[i], self.triple_z[i]);
            }
            let x_star = self.triple_z[self.tree_n] ^ beta_mul_chialpha;
            let x_star_bytes = x_star.to_le_bytes();
            *comm += io.send_block::<16>(&[x_star_bytes]).expect("Cannot send x_star.");

            let mut va = 0u128;
            va ^= self.triple_y[self.tree_n];
            for i in 0..self.tree_n {
                va ^= self.check_vw_buf[i];
            }

            let hash = blake3::hash(&va.to_le_bytes());
            let mut h = [0u8; 32];
            h.copy_from_slice(hash.as_bytes());

            let r = io.receive_block::<32>().expect("Cound not receive h from Sender")[0];

            if r != h {
                panic!("Consistency check for Mpfss failed!");
            }
        }

    }

    pub fn seed_expand<IO: CommunicationChannel>(&mut self, io: &mut IO, seed: &mut [u128], threads: usize, comm: &mut u64) {
        let mut sd = [0u8; 16];
        if self.party == 0 {
            sd = io.receive_block::<16>().expect("Failed to receive seed")[0];
        } else {
            let mut sd_buf = vec![[0u8; 16]; 1];
            self.prg.random_16byte_block(&mut sd_buf);
            sd = sd_buf[0].clone();
            *comm += io.send_block::<16>(&[sd]).expect("Failed to send seed");
        }
        let mut prg2 = PRG::new(Some(&sd), 0);
        let mut seed_bytes = vec![[0u8; 16]; seed.len()];
        prg2.random_16byte_block(&mut seed_bytes);
        for i in 0..threads {
            seed[i] = u128::from_le_bytes(seed_bytes[i]);
        }
    }
}