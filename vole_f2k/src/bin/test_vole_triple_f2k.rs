extern crate psi_volef2k;
extern crate psi_network;

use psi_network::tcp_channel::TcpChannel;
use psi_volef2k::vole_triple_f2k::VoleTripleF2k;
use psi_volef2k::vole_triple_f2k::LPN16;
use psi_volef2k::utils_f2k::rand_u128;
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    let mut comm: u64 = 0;
    const SIZE: usize = 100_000;

    if role == "receiver" {
        // Receiver setup
        let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to bind to port");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);

        let mut vole = VoleTripleF2k::new(1, true, &mut channel, LPN16, &mut comm);

        let start = std::time::Instant::now();
        vole.setup_receiver(&mut channel, &mut comm);
        println!("Time taken for setup: {:?}", start.elapsed());

        vole.extend_initialization();
        let mut y = vec![0u128; SIZE];
        let mut z = vec![0u128; SIZE];

        let start = Instant::now();
        vole.extend(&mut channel, &mut y, &mut z, SIZE, &mut comm);
        println!("Time taken for one extend: {:?}", start.elapsed());

        vole.check_triple(&mut channel, 0u128, &y, &z, SIZE);
    } else if role == "sender" {
        // Sender setup
        let stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);

        let mut vole = VoleTripleF2k::new(0, true, &mut channel, LPN16, &mut comm);

        let delta = rand_u128();
        vole.setup_sender(&mut channel, delta, &mut comm);
        vole.extend_initialization();

        let mut y = vec![0u128; SIZE];
        let mut z = vec![0u128; SIZE];
        vole.extend(&mut channel, &mut y, &mut z, SIZE, &mut comm);

        vole.check_triple(&mut channel, delta, &y, &z, SIZE);
    }

    println!("Total communication: {}", comm);
}