extern crate psi_network;
extern crate psi_volef2k;

use psi_network::tcp_channel::{connect_with_retry_tcp, listen_tcp};
use psi_volef2k::base_svole_f2k::BaseSvoleF2k;
use psi_volef2k::utils_f2k::rand_u128;
use std::time::Instant;

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    const BATCH_SIZE: usize = 200;

    if role == "receiver" {
        // Receiver logic
        let mut channel = listen_tcp("127.0.0.1:8080").expect("Failed to bind to address");
        println!("Waiting for sender...");
        let mut receiver_svole = BaseSvoleF2k::new_receiver(&mut channel);
        let mut shares = vec![0u128; BATCH_SIZE];
        let mut u_batch = vec![0u128; BATCH_SIZE];

        let start = Instant::now();
        receiver_svole.triple_gen_recv(&mut channel, &mut shares, &mut u_batch, BATCH_SIZE);
        let duration = start.elapsed();
        println!("Triple generation (recv) time: {:?}", duration);
    } else if role == "sender" {
        // Sender logic
        let mut channel = connect_with_retry_tcp("127.0.0.1:8080").expect("Failed to connect to receiver");
        let delta = rand_u128();
        println!("Sender Delta: {}", delta);
        let mut sender_svole = BaseSvoleF2k::new_sender(&mut channel, delta);
        let mut shares = vec![0u128; BATCH_SIZE];
        let start = Instant::now();
        sender_svole.triple_gen_send(&mut channel, &mut shares, BATCH_SIZE);
        let duration = start.elapsed();
        println!("Triple generation (send) time: {:?}", duration);
    }
}