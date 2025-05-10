extern crate psi_okvs;
extern crate rand;

use psi_okvs::okvs_f2k::RbOkvsF2k;
use psi_okvs::types::Pair;
use std::time::Instant;
use rand::RngCore;

pub fn rand_u128() -> u128 {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    u128::from_le_bytes(bytes)
}

fn main() {
    let size = 1000; // 1 million elements

    let mut inputs: Vec<Pair<u128, u128>> = Vec::with_capacity(size);
    for i in 0..size {
        inputs.push((rand_u128(), rand_u128()));
    }

    println!("Inputs: {:?}", &inputs[..5]);

    let r1 = [0u8; 16];
    let r2 = [1u8; 16];

    let okvs = RbOkvsF2k::new(size, &r1, &r2);

    let start = Instant::now();
    let u = okvs.encode(&inputs).unwrap();
    println!("Time to encode OKVS for {} elements: {:?}", size, start.elapsed());

    // println!("Encoded OKVS: {:?}", &u);
    for x in &u[..5] {
        println!("{:?}", x);
    }

    let keys = inputs.iter().map(|x| x.0.clone()).collect::<Vec<u128>>();
    let decoded = okvs.decode(&u, &keys);


    for i in 0..size {
        assert_eq!(inputs[i].1, decoded[i], "Mismatch at index {}", i);
    }
}