extern crate blake3;
extern crate rand;

use rand::prelude::*;
use std::time::Instant;

fn main() {
    let mut rng = ThreadRng::default();
    for i in (1..1000).step_by(25) {
        let mut input = vec![0u8; i];
        rng.fill_bytes(&mut input);
        let start = Instant::now();
        let _hsh = blake3::hash(&input);
        println!("Hashing time for {} bytes: {:?}", i, start.elapsed());
    }
}