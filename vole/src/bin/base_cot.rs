extern crate psi_vole;

use psi_vole::comm_channel::CommunicationChannel;
use psi_vole::socket_channel::TcpChannel;
use psi_vole::base_cot::BaseCot;
use psi_vole::preot::OTPre;
use std::net::TcpStream;
use std::env;
use std::net::TcpListener;
use std::time::Instant;

fn main() {
    // Get the role argument (sender or receiver)
    let role = env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    let mut comm: u64 = 0;

    if role == "receiver" {
        // Listen for the sender
        let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to bind to address");
        println!("Waiting for sender...");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);

        // Initialize BaseCot for the receiver (BOB)
        let mut receiver_cot = BaseCot::new(1, false);

        // Set up the receiver's precomputation phase
        receiver_cot.cot_gen_pre(&mut channel, None, &mut comm);

        // Original COT generation
        let size = 60; // Number of COTs
        let times = 100;
        let mut original_ot_data = vec![[0u8; 32]; size];
        let mut choice_bits = vec![false; size];

        let mut receiver_pre_ot = OTPre::new(size, times);
        receiver_cot.cot_gen_preot(&mut channel, &mut receiver_pre_ot, size * times, None, &mut comm);

        let start = Instant::now();
        for s in 0..times {
            receiver_pre_ot.choices_recver(&mut channel, &choice_bits, &mut comm);
        }
        channel.flush();
        receiver_pre_ot.reset();
        // Receive data using OTPre
        for s in 0..times {
            let mut received_data = vec![[0u8; 32]; size];
            receiver_pre_ot.recv(&mut channel, &mut received_data, &choice_bits, size, s, &mut comm);
        }
        let duration = start.elapsed();
        println!("Time taken: {:?}", duration);
    } else if role == "sender" {
        // Connect to the receiver
        let stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);

        // Initialize BaseCot for the sender (ALICE)
        let mut sender_cot = BaseCot::new(0, false);

        // Set up the sender's precomputation phase
        sender_cot.cot_gen_pre(&mut channel, None, &mut comm);

        // Original COT generation
        let size = 60; // Number of COTs
        let times = 100;
        let mut original_ot_data = vec![[0u8; 32]; size];

        let mut sender_pre_ot = OTPre::new(size, times);
        sender_cot.cot_gen_preot(&mut channel, &mut sender_pre_ot, size*times, None, &mut comm);
        for s in 0..times {
            sender_pre_ot.choices_sender(&mut channel, &mut comm);
        }
        channel.flush();
        sender_pre_ot.reset();

        for s in 0..times {
            // Send data using OTPre
            let mut m0 = vec![[0u8; 32]; size];
            let mut m1 = vec![[0u8; 32]; size];
            for i in 0..size {
                m0[i] = [i as u8; 32];
                m1[i] = [(i + 1) as u8; 32];
            }
            sender_pre_ot.send(&mut channel, &m0, &m1, size, s, &mut comm);
        }
    } else {
        panic!("Invalid role specified. Please specify 'sender' or 'receiver'.");
    }

    println!("Total data sent: {} bytes", comm);
}
