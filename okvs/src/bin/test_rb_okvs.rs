extern crate psi_okvs;
extern crate lambdaworks_math;
extern crate rand;

use psi_okvs::okvs::{RbOkvs};
use psi_okvs::types::{Pair, Okvs};
use psi_okvs::utils::rand_field_element;
use lambdaworks_math::field::fields::fft_friendly::stark_252_prime_field::Stark252PrimeField;
use lambdaworks_math::field::element::FieldElement;
use std::time::Instant;

pub type F = Stark252PrimeField;
pub type FE = FieldElement<F>;

fn main() {
    let size = 1 << 20;    

    let mut inputs: Vec<Pair<FE, FE>> = Vec::with_capacity(size);
    for i in 0..size {
        inputs.push((rand_field_element(), rand_field_element()));
    }

    println!("Inputs: {:?}", &inputs[..5]);

    let r1 = [0u8; 16];
    let r2 = [1u8; 16];

    let okvs = RbOkvs::new(size, &r1, &r2);

    let start = Instant::now();
    let u = okvs.encode(&inputs).unwrap();
    println!("Time to encode OKVS for {} elements: {:?}", size, start.elapsed());

    for x in &u[..5] {
        println!("{:?}", x);
    }

    let keys = inputs.iter().map(|x| x.0.clone()).collect::<Vec<FE>>();

    let decoded = okvs.decode(&u, &keys);

    for i in 0..size {
        assert_eq!(inputs[i].1, decoded[i], "Mismatch at index {}", i);
    }
}
