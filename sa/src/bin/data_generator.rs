extern crate psi_sa;
extern crate clap;
extern crate rand;

use psi_sa::config::*;
use clap::Parser;
use rand::{rngs::ThreadRng, Rng};
use std::{
    fs::File,
    io::Write,
};

fn get_origin(point: &[u128; DIMENSION]) -> [u128; DIMENSION] {
    let mut origin = [0u128; DIMENSION];
    for i in 0..DIMENSION {
        origin[i] = point[i];
        origin[i] = (origin[i] >> RANGE_BITS) << RANGE_BITS;
    }
    origin
}


fn gen_origins(rng: &mut ThreadRng, set_size: usize) -> Vec<[u128; DIMENSION]> {
    // We will generate two sets with the same set of origins to make sure there are intersections

    let mut origins_set: std::collections::HashSet<[u128; DIMENSION]> = std::collections::HashSet::new();

    while origins_set.len() < set_size {
        let point = rng.random::<[u128; DIMENSION]>();
        origins_set.insert(get_origin(&point));
    }

    let mut origins: Vec<[u128; DIMENSION]> = Vec::new();
    origins_set.iter().for_each(|point| {
        origins.push(*point);
    });

    origins
}

fn balls_into_bins(
    rng: &mut ThreadRng, 
    num_bins: usize, 
    num_balls: usize, 
    max_per_bin: usize)
-> (Vec<usize>, usize) {
    let mut bins = vec![0usize; num_bins];
    let mut balls_placed = 0;   
    while balls_placed < num_balls {
        let bin = rng.random_range(0..num_bins);
        if bins[bin] < max_per_bin {
            bins[bin] += 1;
            balls_placed += 1;
        }
    }
    let num_nonzero_bins = bins.iter().filter(|&&x| x > 0).count();
    (bins, num_nonzero_bins)
}

fn in_range(point1: [u128; DIMENSION], point2: [u128; DIMENSION]) -> bool {
    for dim in 0..DIMENSION {
        if (point2[dim] + (RADIUS as u128) < point1[dim]) || (point2[dim] > point1[dim] + (RADIUS as u128)) {
            return false;
        }
    }
    true
}

fn gen_inputs(
    rng: &mut ThreadRng, 
    origins: &Vec<[u128; DIMENSION]>, 
    set_size: usize, 
    intersection_size: usize)
-> (Vec<[u128; DIMENSION]>, Vec<[u128; DIMENSION]>) {
    // We will generate input as follow:
    // For sender, we generate a random point around each origin
    // For receiver, we make sure that:
    // 1. Each origin has at most one intersection. 
    // 2. Total number of intersections is intersection_size
    // 3. Each origin has random number of points around it too
    // 4. Total number of points is set_size

    // First assert that the number of origins is equal to set_size
    assert_eq!(origins.len(), set_size, "Number of origins must be equal to set_size");

    // We generate sender inputs first
    let mut sender_inputs: Vec<[u128; DIMENSION]> = Vec::new();
    for origin in origins.iter() {
        // Generate a random point around the origin
        let mut point = rng.random::<[u128; DIMENSION]>();
        for i in 0..DIMENSION {
            point[i] = (point[i] % (1u128 << RANGE_BITS)) + origin[i];
        }
        sender_inputs.push(point);
    }

    println!("Generated {} sender inputs", sender_inputs.len());

    // How do I generate zipf?
    let (mini_universe_sizes, num_nonzero_bins) = balls_into_bins(rng, origins.len(), set_size, 5);
    println!("Generated mini universe sizes. Number of nonzero bins: {}", num_nonzero_bins);
    assert!(num_nonzero_bins >= intersection_size, "Not enough non-zero bins to place intersections, try increasing set_size or decreasing intersection_size");

    let (intersect_bins, _num_nonzero_bins_intersect) = balls_into_bins(rng, num_nonzero_bins, intersection_size, 1);
    // Now try to assign intersections
    let mut mini_universe_intersection_sizes = vec![0usize; origins.len()];
    let mut intersect_bin_index = 0;
    for i in 0..origins.len() {
        if mini_universe_sizes[i] > 0 {
            if intersect_bins[intersect_bin_index] > 0 {
                mini_universe_intersection_sizes[i] = 1;
            }
            intersect_bin_index += 1;
        }
    }

    // We are now ready
    let mut receiver_inputs: Vec<[u128; DIMENSION]> = Vec::new();
    for (i, origin) in origins.iter().enumerate() {
        // First add intersection if any
        if mini_universe_intersection_sizes[i] == 1 {
            // Generate the same point as sender
            let point = loop {
                let mut p = rng.random::<[u128; DIMENSION]>();
                for d in 0..DIMENSION {
                    p[d] = (p[d] % (1u128 << RANGE_BITS)) + origin[d];
                }
                if in_range(p, sender_inputs[i]) {
                    break p;
                }
            };
            receiver_inputs.push(point);
        }

        for _ in 0..(mini_universe_sizes[i] - mini_universe_intersection_sizes[i]) {
            let point = loop {
                let mut p = rng.random::<[u128; DIMENSION]>();
                for d in 0..DIMENSION {
                    p[d] = (p[d] % (1u128 << RANGE_BITS)) + origin[d];
                }
                if !in_range(p, sender_inputs[i]) {
                    break p;
                }
            };
            receiver_inputs.push(point);
        }
    }
    println!("Generated {} receiver inputs", receiver_inputs.len());

    // Placeholder
    (origins.clone(), origins.clone())

}

#[derive(Parser, Debug)]
struct Args {
    #[clap(long)]
    set_size: usize,
    #[clap(long)]
    intersection_size: usize,
    #[clap(long)]
    output_sender: String,
    #[clap(long)]
    output_receiver: String,
}

fn main() {
    let args = Args::parse();

    let set_size = args.set_size;
    let intersection_size = args.intersection_size;
    let output_sender = args.output_sender;
    let output_receiver = args.output_receiver;

    let mut rng = rand::rng();
    let origins = gen_origins(&mut rng, set_size);

    println!("Generated {} origins", origins.len());

    let (sender_inputs, receiver_inputs) = gen_inputs(&mut rng, &origins, set_size, intersection_size);

    // Write to files
    {
        let mut file = File::create(&output_sender).expect("Unable to create sender output file");
        for point in sender_inputs.iter() {
            writeln!(file, "{:?}", point).expect("Unable to write to sender output file");
        }
        println!("Wrote sender inputs to {}", output_sender);
    }
    {
        let mut file = File::create(&output_receiver).expect("Unable to create receiver output file");
        for point in receiver_inputs.iter() {
            writeln!(file, "{:?}", point).expect("Unable to write to receiver output file");
        }
        println!("Wrote receiver inputs to {}", output_receiver);
    }
}