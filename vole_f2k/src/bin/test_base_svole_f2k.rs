extern crate psi_network;
extern crate psi_volef2k;

use psi_network::socket_channel::TcpChannel;
use psi_volef2k::base_svole_f2k::BaseSvoleF2k;
use psi_volef2k::utils_f2k::rand_u128;
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    let mut comm: u64 = 0;
    let batch_size = 200;

    if role == "receiver" {
        // Receiver logic
        let listener = std::net::TcpListener::bind("127.0.0.1:8080").expect("Failed to bind to address");
        println!("Waiting for sender...");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);
        let mut receiver_svole = BaseSvoleF2k::new_receiver(&mut channel, &mut comm);
        let batch_size = 200;
        let mut shares = vec![0u128; batch_size];
        let mut u_batch = vec![0u128; batch_size];

        let start = Instant::now();
        receiver_svole.triple_gen_recv(&mut channel, &mut shares, &mut u_batch, batch_size, &mut comm);
        let duration = start.elapsed();
        println!("Triple generation (recv) time: {:?}", duration);
    } else if role == "sender" {
        // Sender logic
        let stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);
        let delta = rand_u128();
        println!("Sender Delta: {}", delta);
        let mut sender_svole = BaseSvoleF2k::new_sender(&mut channel, delta, &mut comm);
        let batch_size = 200;
        let mut shares = vec![0u128; batch_size];
        let start = Instant::now();
        sender_svole.triple_gen_send(&mut channel, &mut shares, batch_size, &mut comm);
        let duration = start.elapsed();
        println!("Triple generation (send) time: {:?}", duration);
    }
}