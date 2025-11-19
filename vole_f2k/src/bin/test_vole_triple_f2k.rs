extern crate psi_volef2k;
extern crate psi_network;

use psi_network::tcp_channel::{connect_with_retry_tcp, listen_tcp};
use psi_volef2k::vole_triple_f2k::VoleTripleF2k;
use psi_volef2k::vole_triple_f2k::LPN16;
use psi_volef2k::utils_f2k::rand_u128;
use std::time::Instant;

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    const SIZE: usize = 100_000;

    if role == "receiver" {
        // Receiver setup
        let mut channel = listen_tcp("127.0.0.1:8080").expect("Failed to bind to port");

        let mut vole = VoleTripleF2k::new(1, true, &mut channel, LPN16);

        let start = std::time::Instant::now();
        vole.setup_receiver(&mut channel);
        println!("Time taken for setup: {:?}", start.elapsed());

        vole.extend_initialization();
        let mut y = vec![0u128; SIZE];
        let mut z = vec![0u128; SIZE];

        let start = Instant::now();
        vole.extend(&mut channel, &mut y, &mut z, SIZE);
        println!("Time taken for one extend: {:?}", start.elapsed());

        vole.check_triple(&mut channel, 0u128, &y, &z, SIZE);

        println!("Receiver communication: {:?}", channel.get_bytes_sent())
    } else if role == "sender" {
        // Sender setup
        let mut channel = connect_with_retry_tcp("127.0.0.1:8080").expect("Failed to connect to receiver");

        let mut vole = VoleTripleF2k::new(0, true, &mut channel, LPN16);

        let delta = rand_u128();
        vole.setup_sender(&mut channel, delta);
        vole.extend_initialization();

        let mut y = vec![0u128; SIZE];
        let mut z = vec![0u128; SIZE];
        vole.extend(&mut channel, &mut y, &mut z, SIZE);

        vole.check_triple(&mut channel, delta, &y, &z, SIZE);

        println!("Sender communication: {:?}", channel.get_bytes_sent())
    }

}