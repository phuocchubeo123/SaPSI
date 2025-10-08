extern crate psi_network;
extern crate psi_volef2k;
extern crate rand;
extern crate rand_chacha;

use psi_network::socket_channel::TcpChannel;
use psi_network::comm_channel::CommunicationChannel;
use psi_volef2k::oprf_sender_f2k::OprfSenderF2k;
use psi_volef2k::oprf_receiver_f2k::OprfReceiverF2k;
use psi_volef2k::vole_triple_f2k::LPN16;
use std::net::{TcpListener, TcpStream};
use std::convert::TryInto;
use rand::prelude::*;
use rand_chacha::rand_core::{SeedableRng, RngCore};
use rand_chacha::ChaCha12Rng;


pub fn gen_input(rng: &mut ChaCha12Rng) -> u128 {
    let u = rng.next_u64();
    let v = rng.next_u64();
    (u as u128) ^ ((v as u128) << 64)
}

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");

    const size: usize = 1 << 5;
    const KEY_DIM: usize = 2;
    let mut comm: u64 = 0;

    if role == "receiver" {
        // Receiver logic
        // Listen for the sender
        let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to bind to port");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);

        let seed = channel.receive_block::<32>().expect("Failed to receive seed from sender");
        let mut rng = ChaCha12Rng::from_seed(seed[0]);
        let data = (0..size).map(|_| {
            let x: [u128; KEY_DIM] = (0..KEY_DIM).map(|_| gen_input(&mut rng)).collect::<Vec<u128>>().try_into().unwrap();
            x
        }).collect::<Vec<[u128; KEY_DIM]>>();

        let mut oprf = OprfReceiverF2k::<KEY_DIM>::new(&mut channel, size, LPN16, &mut comm);
        oprf.receive(&mut channel, &data, &mut comm);

        data.iter().for_each(|x| {
            println!("Query for {:?}: {:?}", x, oprf.get_output(x));
        })
    } else if role == "sender" {
        // Sender logic
        // Connect to the receiver
        let stream = TcpStream::connect("127.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);

        // Send data to Sender for test
        let mut seed = [0u8; 32];
        let mut rng_seed = rand::thread_rng();
        rng_seed.fill(&mut seed);
        channel.send_block::<32>(&[seed]).expect("Failed to send seed to receiver");
        let mut rng = ChaCha12Rng::from_seed(seed);
        let data = (0..2*size).map(|_| {
            let x: [u128; KEY_DIM] = (0..KEY_DIM).map(|_| gen_input(&mut rng)).collect::<Vec<u128>>().try_into().unwrap();
            x
        }).collect::<Vec<[u128; KEY_DIM]>>();

        let mut oprf = OprfSenderF2k::<KEY_DIM>::new(&mut channel, size, LPN16, &mut comm);
        oprf.send(&mut channel, &data, &mut comm);

        data.iter().for_each(|x| {
            println!("Query for {:?}: {:?}", x, oprf.get_output(x));
        })
    }
}