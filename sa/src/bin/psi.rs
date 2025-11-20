extern crate psi_sa;
extern crate psi_network;
extern crate psi_volef2k;
extern crate rand;
extern crate rand_chacha;
extern crate clap;

use psi_sa::config::*;
use psi_sa::psi_sender::SAPSISender;
use psi_sa::psi_receiver::SAPSIReceiver;
use psi_network::tcp_channel::{connect_with_retry_tcp, listen_tcp};
use psi_volef2k::vole_triple_f2k::{LPN12, LPN16, LPN20, PrimalLPNParameterF2k};
use std::time::Instant;
use clap::Parser;

fn param_checking(size: usize) -> PrimalLPNParameterF2k{
    let param = if size == 1 << 8 {
            LPN12
        } else if size == 1 << 12 {
            LPN16
        } else if size == 1 << 16 {
            LPN20
        } else {
            panic!("Invalid size, only accept 2^8, 2^12, or 2^16");
        };
    if RADIUS == 10 {
        if RANGE_BITS != 6 {
            panic!("RADIUS = 10, but RANGE_BITS != 6");
        } 
        if PREF_LENGTH != [2, 4, 6] {
            panic!("RADIUS = 10, but PREF_LENGTH != [2, 4, 6]");
        }
    } else if RADIUS == 30 {
        if RANGE_BITS != 7 {
            panic!("RADIUS = 30, but RANGE_BITS != 7");
        }
        if PREF_LENGTH != [1, 3, 5, 7] {
            panic!("RADIUS = 30, but PREF_LENGTH != [1, 3, 5, 7]");
        }
    } else if RADIUS == 60 {
        if RANGE_BITS != 8 {
            panic!("RADIUS = 60, but RANGE_BITS != 8");
        }
        if PREF_LENGTH != [2, 4, 6, 8] {
            panic!("RADIUS = 60, but PREF_LENGTH != [2, 4, 6, 8]");
        }
    } else if RADIUS == 120 {
        if RANGE_BITS != 9 {
            panic!("RADIUS = 120, but RANGE_BITS != 9");
        }
        if PREF_LENGTH != [1, 3, 5, 7, 9] {
            panic!("RADIUS = 120, but PREF_LENGTH != [1, 3, 5, 7, 9]");
        }
    } else if RADIUS == 250 {
        if RANGE_BITS != 10 {
            panic!("RADIUS = 250, but RANGE_BITS != 10");
        }
        if PREF_LENGTH != [2, 4, 6, 8, 10] {
            panic!("RADIUS = 250, but PREF_LENGTH != [2, 4, 6, 8, 10]");
        }
    } else {
        panic!("Invalid RADIUS");
    }

    param
}

fn load_points_from_file(file_path: &str) -> Vec<[u128; DIMENSION]> {
    let mut points: Vec<[u128; DIMENSION]> = Vec::new();
    let content = std::fs::read_to_string(file_path).expect("Unable to read file");
    for line in content.lines() {
        let trimmed = line.trim_matches(|c: char| c == '[' || c == ']' || c.is_whitespace());
        let nums: Vec<u128> = trimmed.split(',').map(|s| s.trim().parse().expect("Unable to parse number")).collect();
        if nums.len() != DIMENSION {
            panic!("Invalid point dimension in file");
        }
        let mut point = [0u128; DIMENSION];
        for i in 0..DIMENSION {
            point[i] = nums[i];
        }
        points.push(point);
    }
    points
}

#[derive(Parser)]
struct Args {
    #[clap(long)]
    role: String,
    #[clap(long)]
    address: String,
    #[clap(long)]
    port: String,
    #[clap(long)]
    size: usize,
    #[clap(long)]
    input_file: String,
}

fn main() {
    let args = Args::parse();
    let role = args.role;
    let address = args.address;
    let port = args.port;
    let size = args.size;
    let table_size = ((size as f32) * 1.5) as usize;
    let input_file = args.input_file;

    assert_eq!(size, N, "Size must be equal to N defined in config.rs");

    println!("Running PSI with size: {}, table_size: {}, radius: {}, dimension: {}", size, table_size, RADIUS, DIMENSION);

    let param = param_checking(size);
    let data = load_points_from_file(&input_file);

    println!("Is there duplicates in input data? {}", {
        let mut set = std::collections::HashSet::new();
        let mut has_duplicates = false;
        for point in data.iter() {
            if !set.insert(point) {
                has_duplicates = true;
                break;
            }
        }
        has_duplicates
    });


    if role == "receiver" {
        println!("Starting as Receiver...");
        let mut channel = listen_tcp(&format!("{}:{}", address, port)).expect("Failed to listen on address");
        let start = Instant::now();

        let receiver_psi = SAPSIReceiver::new(table_size);
        receiver_psi.receive(&mut channel, &data, param);

        println!("Receiver finished in {:?}", start.elapsed());
        println!("Total communication: {} bytes", channel.get_bytes_sent());
    } else if role == "sender" {
        let mut channel = connect_with_retry_tcp(&format!("{}:{}", address, port)).expect("Failed to connect to receiver");
        let start = Instant::now();

        let mut sender_psi = SAPSISender::new(table_size);
        sender_psi.send(&mut channel, &data, param);

        println!("Sender finished in {:?}", start.elapsed());
        println!("Total communication: {} bytes", channel.get_bytes_sent());
    }
}
