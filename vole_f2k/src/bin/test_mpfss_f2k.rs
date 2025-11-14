extern crate psi_network;
extern crate psi_ot;
extern crate psi_volef2k;
extern crate psi_utils;

use psi_network::tcp_channel::TcpChannel;
use psi_network::comm_channel::CommunicationChannel;    
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_volef2k::base_svole_f2k::BaseSvoleF2k;
use psi_volef2k::mpfss_reg_f2k::MpfssRegF2k;
use psi_volef2k::utils_f2k::rand_u128;
use psi_utils::gf128::gf128mul;
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

fn main() {
    let role = std::env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");

    const log_bin_sz: usize = 4;
    const t: usize = 100;
    const n: usize = t * (1 << log_bin_sz);
    const k: usize = 2;

    let mut comm: u64 = 0;

    if role == "receiver" {
        // Receiver logic
        // Listen for the sender
        let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to bind to port");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);

        // Initialize BaseCot for the receiver (BOB)
        let mut receiver_cot = BaseCot::new(1, false);
        // Set up the receiver's precomputation phase
        receiver_cot.cot_gen_pre(&mut channel, None, &mut comm);
        let mut pre_ot = OTPre::<1>::new(log_bin_sz, t);
        receiver_cot.cot_gen_preot(&mut channel, &mut pre_ot, log_bin_sz * t, None, &mut comm);

        let mut mac = vec![0u128; t + 1];
        let mut u = vec![0u128; t + 1];

        let mut svole = BaseSvoleF2k::new_receiver(&mut channel, &mut comm);
        // mac = key + delta * u
        svole.triple_gen_recv(&mut channel, &mut mac, &mut u, t + 1, &mut comm);

        // Try to test mac = key + delta * u first
        let mac_bytes = mac.iter().map(|&x| x.to_le_bytes()).collect::<Vec<_>>();
        let u_bytes = u.iter().map(|&x| x.to_le_bytes()).collect::<Vec<_>>();
        channel.send_block::<16>(&mac_bytes).expect("Failed to send mac");
        channel.send_block::<16>(&u_bytes).expect("Failed to send u");


        let mut y = vec![0u128; n];
        let mut z = vec![0u128; n];
        let mut mpfss = MpfssRegF2k::new(n, t, log_bin_sz, 1);
        mpfss.set_malicious();

        mpfss.receiver_init();
        mpfss.mpfss_receiver(&mut channel, &mut pre_ot, &mac, &u, &mut y, &mut z, &mut comm);
    } else {
        // Sender logic
        // Connect to the receiver
        let stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);
        // Initialize BaseCot for the sender (ALICE)
        let mut sender_cot = BaseCot::new(0, false);
        // Set up the sender's precomputation phase
        sender_cot.cot_gen_pre(&mut channel, None, &mut comm);
        let mut pre_ot = OTPre::<1>::new(log_bin_sz, t);
        sender_cot.cot_gen_preot(&mut channel, &mut pre_ot, log_bin_sz * t, None, &mut comm);
        let delta = rand_u128();
        let mut key = vec![0u128; t + 1];
        let mut svole = BaseSvoleF2k::new_sender(&mut channel, delta, &mut comm);
        svole.triple_gen_send(&mut channel, &mut key, t + 1, &mut comm);

        let mac_bytes = channel.receive_block::<16>().expect("Failed to receive mac");
        let u_bytes = channel.receive_block::<16>().expect("Failed to receive u");
        let mac: Vec<u128> = mac_bytes.iter().map(|&x| u128::from_le_bytes(x)).collect();
        let u: Vec<u128> = u_bytes.iter().map(|&x| u128::from_le_bytes(x)).collect();
        // Test mac = key + delta * u
        for i in 0..t + 1 {
            let mut check = mac[i] ^ key[i];
            check ^= gf128mul(delta, u[i]);
            assert_eq!(check, 0, "MAC verification failed at index {}", i);
        }

        let mut y = vec![0u128; n];
        let mut mpfss = MpfssRegF2k::new(n, t, log_bin_sz, 0);
        mpfss.set_malicious();

        mpfss.sender_init(delta);

        let start = Instant::now();
        mpfss.mpfss_sender(&mut channel, &mut pre_ot, &key, &mut y, &mut comm);
        let duration = start.elapsed();
        println!("Time taken to generate {} Spfss: {:?}", t, duration);
    }
}