use psi_network::tcp_channel::TcpChannel;
use psi_aes::prg::PRG;
use psi_utils::gf128::{vector_inner_product_f2k, gf128mul};

use crate::cope_f2k::CopeF2k;

pub struct BaseSvoleF2k {
    party: bool,
    cope: CopeF2k,         // COPE instance
    delta: Option<u128>,     // Delta for the sender
}

impl BaseSvoleF2k {
    /// Sender's constructor
    pub fn new_sender(io: &mut TcpChannel, delta: u128) -> Self {
        let mut cope = CopeF2k::new(0);
        cope.initialize_sender(io, delta.clone());
        Self {
            party: true,
            cope,
            delta: Some(delta),
        }
    }

    /// Receiver's constructor
    pub fn new_receiver(io: &mut TcpChannel) -> Self {
        let mut cope = CopeF2k::new(1);
        cope.initialize_receiver(io);
        Self {
            party: false,
            cope,
            delta: None,
        }
    }

    pub fn triple_gen_send(&mut self, io: &mut TcpChannel, share: &mut [u128], size: usize) {
        assert_eq!(self.party, true, "Only sender can call triple_gen_send");
        // Generate share_recv = share_send + delta * u_recv
        self.cope.extend_sender_batch(io, share, size);
        let mut b = vec![0u128; 1];
        self.cope.extend_sender_batch(io, &mut b, 1);
        self.sender_check(io, share, b[0], size);
    }

    pub fn triple_gen_recv(&mut self, io: &mut TcpChannel, share: &mut [u128], u: &mut [u128], size: usize) {
        assert_eq!(self.party, false, "Only receiver can call triple_gen_recv");
        // Generate share_recv = share_send + delta * u_recv
        let mut prg = PRG::new(None, 0);
        let mut x_bytes = vec![[0u8; 16]; 1];
        let mut u_bytes = vec![[0u8; 16]; u.len()];
        prg.random_16byte_block(&mut x_bytes);
        prg.random_16byte_block(&mut u_bytes);

        let x = x_bytes
            .iter()
            .map(|x| u128::from_le_bytes(*x))
            .collect::<Vec<u128>>();
        u.copy_from_slice(&u_bytes
            .iter()
            .map(|x| u128::from_le_bytes(*x))
            .collect::<Vec<u128>>());

        self.cope.extend_receiver_batch(io, share, u, size);

        let mut c = vec![0u128; 1];
        self.cope.extend_receiver_batch(io, &mut c, &x, 1);

        self.receiver_check(io, share, u, c[0], x[0], size);
    }

    /// Sender: Consistency check
    fn sender_check(&mut self, io: &mut TcpChannel, share: &[u128], b: u128, size: usize) {
        assert_eq!(self.party, true, "Only sender can call sender_check");
        // Generate check seed and send it to Receiver
        let mut seed = vec![[0u8; 16]; 1];
        let mut seed_prg = PRG::new(None, 0);
        seed_prg.random_16byte_block(&mut seed);
        io.send_block::<16>(&seed).expect("Send seed for svole check failed");

        let chi = self.generate_hash_coeff(seed[0], size);

        let y = vector_inner_product_f2k(share, &chi) ^ b;
        let xz_bytes = io.receive_block::<16>().expect("Failed to receive xz");
        let mut xz = xz_bytes
            .iter()
            .map(|x| u128::from_le_bytes(*x))
            .collect::<Vec<u128>>();

        xz[1] = gf128mul(xz[1], self.delta.unwrap());
        let y_check = y ^ xz[1];
        if y_check != xz[0] {
            panic!("Base sVOLE check failed!");
        } else {
            println!("Base sVOLE generated successfully!");
        }
    }

    fn receiver_check(&mut self, io: &mut TcpChannel, share: &[u128], x: &[u128], c: u128, a: u128, size: usize) {
        assert_eq!(self.party, false, "Only receiver can call receiver_check");
        let seed = io.receive_block::<16>().expect("Cannot receive seed for check base sVOLE");
        let chi = self.generate_hash_coeff(seed[0], size);

        let xz_0 = vector_inner_product_f2k(share, &chi) ^ c;
        let xz_1 = vector_inner_product_f2k(x, &chi) ^ a;

        io.send_block::<16>(&[xz_0.to_le_bytes(), xz_1.to_le_bytes()]).expect("Failed to send xz");
    }

    /// Generate hash coefficients based on a seed
    fn generate_hash_coeff(&self, seed: [u8; 16], size: usize) -> Vec<u128> {
        let mut coeffs_bytes = vec![[0u8; 16]; size];
        let mut prg = PRG::new(Some(&seed), 0);
        prg.random_16byte_block(&mut coeffs_bytes);
        let coeffs = coeffs_bytes
            .iter()
            .map(|x| u128::from_le_bytes(*x))
            .collect::<Vec<u128>>();
        coeffs
    }
}