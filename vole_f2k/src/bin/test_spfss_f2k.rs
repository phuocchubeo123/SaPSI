extern crate psi_ot;
extern crate psi_utils;
extern crate psi_volef2k;
extern crate psi_network;
extern crate rand;

use std::time::Instant;
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_utils::gf128::gf128mul;
use psi_network::socket_channel::TcpChannel;
use psi_network::comm_channel::CommunicationChannel;
use psi_volef2k::spfss_sender_f2k::SpfssSenderF2k;
use psi_volef2k::spfss_receiver_f2k::SpfssRecverF2k;
use rand::{RngCore, random};

fn rand_u128() -> u128 {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    u128::from_le_bytes(bytes)
}

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    let mut comm: u64 = 0;

    if role == "receiver" {
        // Receiver logic
        // Listen for the sender
        let listener = std::net::TcpListener::bind("127.0.0.1:8080").expect("Failed to bind to address");
        println!("Waiting for sender...");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);

        // Initialize BaseCot for the receiver (BOB)
        let mut receiver_cot = BaseCot::new(1, false);
        receiver_cot.cot_gen_pre(&mut channel, None, &mut comm);

        // Original COT generation
        const depth: usize = 4;
        let size = depth - 1; // Number of COTs
        let times = 100;
        let mut choice_bits = vec![false; size * times];
        // Populate random choice bits
        for bit in &mut choice_bits {
            *bit = rand::random();
        }

        // New COT generation using OTPre
        let mut receiver_pre_ot = OTPre::<1>::new(size, times);
        receiver_cot.cot_gen_preot(&mut channel, &mut receiver_pre_ot, size * times, Some(&choice_bits), &mut comm);

        let delta_bytes = channel.receive_block::<16>().expect("Failed to receive delta");
        let delta = u128::from_le_bytes(delta_bytes[0]);
        let gamma_bytes = channel.receive_block::<16>().expect("Failed to receive gamma");
        let gamma = u128::from_le_bytes(gamma_bytes[0]);

        let mut ggm_tree_mem = [0u128; 1 << (depth - 1)];
        for i in 0..times {
            receiver_pre_ot.choices_recver(&mut channel, &[false; depth - 1], &mut comm);
        }
        channel.flush();
        receiver_pre_ot.reset();

        for i in 0..times {
            let beta = rand_u128();
            let delta2 = gamma ^ gf128mul(delta, beta);

            let mut receiver_spfss_f2k = SpfssRecverF2k::new(depth);
            receiver_spfss_f2k.recv(&mut channel, &mut receiver_pre_ot, i, &mut comm);
            receiver_spfss_f2k.compute(&mut ggm_tree_mem, delta2);
            // receiver_spfss_f2k.consistency_check(&mut channel, delta2, beta, &mut comm);
        }
    } else if role == "sender" {
        // Connect to the receiver
        let stream = std::net::TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);
        let mut sender_cot = BaseCot::new(0, false);
        sender_cot.cot_gen_pre(&mut channel, None, &mut comm);
        const depth: usize = 4;
        let size = depth - 1; // Number of COTs
        let times = 100;
        let mut choice_bits = vec![false; size * times];
        // Populate random choice bits
        for bit in &mut choice_bits {
            *bit = rand::random();
        }
        let mut sender_pre_ot = OTPre::<1>::new(size, times);
        sender_cot.cot_gen_preot(&mut channel, &mut sender_pre_ot, size * times, Some(&choice_bits), &mut comm);
        let mut delta_bytes = [0u8; 16];
        let mut gamma_bytes = [0u8; 16];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut delta_bytes);
        rng.fill_bytes(&mut gamma_bytes);
        let delta = u128::from_le_bytes(delta_bytes);
        let gamma = u128::from_le_bytes(gamma_bytes);
        channel.send_block::<16>(&[delta_bytes]).expect("Failed to send delta");
        channel.send_block::<16>(&[gamma_bytes]).expect("Failed to send gamma");


        let mut ggm_tree_mem = [0u128; 1 << (depth - 1)];

        let start = Instant::now();
        for i in 0..times {
            sender_pre_ot.choices_sender(&mut channel, &mut comm);
        }
        channel.flush();
        sender_pre_ot.reset();

        for i in 0..times {
            let mut sender_spfss_f2k = SpfssSenderF2k::new(depth);
            sender_spfss_f2k.compute(&mut ggm_tree_mem, delta, gamma);
            sender_spfss_f2k.send(&mut channel, &mut sender_pre_ot, i, &mut comm);
            channel.flush();
        }

        println!("Time taken: {:?}", start.elapsed());
    }

}