extern crate psi_volef2k;
extern crate psi_network;

use psi_network::socket_channel::TcpChannel;
use psi_volef2k::cope_f2k::CopeF2k;
use psi_volef2k::utils_f2k::rand_u128;
use std::time::Instant;

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    let mut comm: u64 = 0;

    if role == "receiver" {
        // Receiver logic
        // Listen for the sender
        let listener = std::net::TcpListener::bind("127.0.0.1:8080").expect("Failed to bind to port");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);

        let mut receiver_cope = CopeF2k::new(1);
        receiver_cope.initialize_receiver(&mut channel, &mut comm);

        let u = rand_u128();

        // Test extend
        let single_result = receiver_cope.extend_receiver(&mut channel, u, &mut comm);
        receiver_cope.check_triple(&mut channel, &[u], &[single_result], 1);

        let start = Instant::now();

        // Test extend_batch
        let batch_size = 20000;
        let u_batch: Vec<u128> = (0..batch_size).map(|_| rand_u128()).collect();
        let mut batch_result = vec![0u128; batch_size];
        receiver_cope.extend_receiver_batch(&mut channel, &mut batch_result, &u_batch, batch_size, &mut comm);

        let duration = start.elapsed();
        println!("Time taken: {:?}", duration);

        receiver_cope.check_triple(&mut channel, &u_batch, &batch_result, batch_size);
    } else if role == "sender" {
        let stream = std::net::TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);

        let mut sender_cope = CopeF2k::new(0);
        let delta = rand_u128();
        sender_cope.initialize_sender(&mut channel, delta, &mut comm);

        // Test extend
        let single_result = sender_cope.extend_sender(&mut channel, &mut comm);
        sender_cope.check_triple(&mut channel, &[delta], &[single_result], 1);

        let start = Instant::now();
        // Test extend_batch
        let batch_size = 20000;
        let mut batch_result = vec![0u128; batch_size];
        sender_cope.extend_sender_batch(&mut channel, &mut batch_result, batch_size, &mut comm);
        let duration = start.elapsed();
        println!("Time taken: {:?}", duration);

        sender_cope.check_triple(&mut channel, &[delta], &batch_result, batch_size);
    }
}