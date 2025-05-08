extern crate psi_sa;
extern crate psi_network;
extern crate rand;
extern crate rand_chacha;

use psi_sa::config::*;
use psi_sa::psi_sender::SAPSISender;
use psi_sa::psi_receiver::SAPSIReceiver;
use psi_network::socket_channel::TcpChannel;
use psi_network::comm_channel::CommunicationChannel;
use std::env;
use std::net::{TcpListener, TcpStream};
use std::collections::HashSet;
use std::time::Instant;
use rand::prelude::*;
use rand_chacha::rand_core::{SeedableRng, RngCore};
use rand_chacha::ChaCha12Rng;

fn gen_input(rng: &mut ChaCha12Rng) -> [u128; DIMENSION] {
    let mut res = [0u128; DIMENSION];
    for i in 0..DIMENSION {
        res[i] = (rng.next_u64() as u128) << 64 | (rng.next_u64() as u128);
    }
    res
}

fn gen_origins(rng: &mut ChaCha12Rng, size: usize) -> Vec<[u128; DIMENSION]> {
    // Generate random origins first
    let pre_origin= (0..size).map(|_| gen_input(rng)).collect::<Vec<[u128; DIMENSION]>>();
    let mut origins_set: HashSet<[u128; DIMENSION]> = HashSet::new();
    pre_origin.iter().for_each(|point| {
        origins_set.insert(get_origin(point));
    });
    let mut origins: Vec<[u128; DIMENSION]> = Vec::new();
    origins_set.iter().for_each(|point| {
        origins.push(*point);
    });
    origins.sort();
    origins
}

fn get_origin(point: &[u128; DIMENSION]) -> [u128; DIMENSION] {
    let mut origin = [0u128; DIMENSION];
    for i in 0..DIMENSION {
        origin[i] = point[i];
        origin[i] = (origin[i] >> RANGE_BITS) << RANGE_BITS;
    }
    origin
}


fn main() {
    let role = env::args().nth(1).expect("Please specify 'sender' or 'receiver' as an argument");
    let mut comm: u64 = 0;

    const size: usize = N;
    const table_size: usize = ((size as f32) * 1.8) as usize;

    if role == "receiver" {
        println!("Starting as Receiver...");
        let listener = TcpListener::bind("127.0.0.1:8080")
            .expect("Failed to bind to port");
        let (stream, _) = listener.accept().expect("Failed to accept connection");
        let mut channel = TcpChannel::new(stream);

        let seed = channel.receive_block::<32>().expect("Failed to receive seed from receiver");
        let mut rng = ChaCha12Rng::from_seed(seed[0]);
        let origins = gen_origins(&mut rng, size);
        let mut data: Vec<[u128; DIMENSION]> = Vec::new();

        let mut intersection_size = 0;

        origins.iter().for_each(|origin| {
            let mut point = gen_input(&mut rng);
            for j in 0..DIMENSION {
                point[j] = point[j] % (1 << RANGE_BITS);
            }
            // println!("Origin: {:?}, Point: {:?}", origin, point);
            for j in 0..DIMENSION {
                point[j] = (point[j] % (1 << RANGE_BITS)) + origin[j];
            }
            data.push(point);

            let mut point2 = [0u128; DIMENSION];
            for dim in 0..DIMENSION {
                let x_bytes = channel.receive_block::<16>().expect("Failed to receive x bytes");
                point2[dim] = u128::from_le_bytes(x_bytes[0]);
            }

            // println!("Point 1: {:?}", point);
            // println!("Point 2: {:?}", point2);

            let mut in_range = true;
            for dim in 0..DIMENSION {
                if (point2[dim] + (RADIUS as u128) < point[dim]) || (point2[dim] > point[dim] + (RADIUS as u128)) {
                    in_range = false;
                    break;
                }
            }
            if in_range {
                intersection_size += 1;
                let mut recentered_point = [0u128; DIMENSION];
                for dim in 0..DIMENSION {
                    recentered_point[dim] = point[dim] - origin[dim];
                }
                let mut recentered_point2 = [0u128; DIMENSION];
                for dim in 0..DIMENSION {
                    recentered_point2[dim] = point2[dim] - origin[dim];
                }
                println!("Intersection: Origin: {:?}, Point: {:?}, Point2: {:?}", origin, recentered_point, recentered_point2);
            }
        });

        println!("Intersection size: {}", intersection_size);

        let start = Instant::now();

        let mut receiver_psi = SAPSIReceiver::new(size, table_size);
        receiver_psi.receive(&mut channel, &data, &mut comm);

        println!("Receiver finished in {:?}", start.elapsed());
        println!("Total communication: {} bytes", comm);
    } else if role == "sender" {
        let stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to receiver");
        let mut channel = TcpChannel::new(stream);

        let mut seed = [2u8; 32]; // debugging with seed 0 first
        // let mut rng_seed = rand::thread_rng();
        // rng_seed.fill(&mut seed);
        let mut rng = ChaCha12Rng::from_seed(seed);
        channel.send_block::<32>(&[seed]).expect("Failed to send seed to sender");

        let origin = gen_origins(&mut rng, size);
        let mut data: Vec<[u128; DIMENSION]> = Vec::new();

        seed = [1u8; 32];
        // rng_seed.fill(&mut seed);
        rng = ChaCha12Rng::from_seed(seed);

        origin.iter().for_each(|origin| {
            let mut point = gen_input(&mut rng);
            point = gen_input(&mut rng);
            for j in 0..DIMENSION {
                point[j] = point[j] % (1 << RANGE_BITS);
            }
            // println!("Origin: {:?}, Point: {:?}", origin, point);
            for j in 0..DIMENSION {
                point[j] = (point[j] % (1 << RANGE_BITS)) + origin[j];
            }
            data.push(point);

            // println!("Point: {:?}", point);

            for dim in 0..DIMENSION {
                let mut x_bytes = [0u8; 16];
                x_bytes.copy_from_slice(&point[dim].to_le_bytes());
                channel.send_block::<16>(&[x_bytes]).expect("Failed to send x bytes");
            }
        });

        let start = Instant::now();

        let mut sender_psi = SAPSISender::new(size, table_size);
        sender_psi.send(&mut channel, &data, &mut comm);

        println!("Sender finished in {:?}", start.elapsed());
        println!("Total communication: {} bytes", comm);
    }
}