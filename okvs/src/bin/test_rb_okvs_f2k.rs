extern crate psi_okvs;
extern crate rand;

use psi_okvs::okvs_f2k::{RbOkvsF2k, Pair};
use std::time::Instant;
use std::convert::TryInto;
use rand::RngCore;

pub fn rand_u128() -> u128 {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    u128::from_le_bytes(bytes)
}

fn main() {
    let size = 1000; // 1 million elements
    const KEY_DIM: usize = 2;

    let mut inputs: Vec<Pair<[u128; KEY_DIM], u128>> = Vec::with_capacity(size);
    for _ in 0..size {
        let key: [u128; KEY_DIM] = (0..KEY_DIM).map(|_| rand_u128()).collect::<Vec<u128>>().try_into().unwrap();
        let value = rand_u128();
        inputs.push((key, value));
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

    let keys = inputs.iter().map(|x| x.0.clone()).collect::<Vec<[u128; KEY_DIM]>>();
    let decoded = okvs.decode(&u, &keys);


    for i in 0..size {
        assert_eq!(inputs[i].1, decoded[i], "Mismatch at index {}", i);
    }
}