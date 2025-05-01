use crate::cuckoo::SimpleHash; 
use crate::idcf_sender::IDCFSender;
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_network::comm_channel::CommunicationChannel;
use rand::prelude::*;
// use sha3::{Digest, Sha3_256};
use blake2::{Blake2s, Blake2s256, Digest};
use std::collections::HashSet;
use std::time::Instant;
use std::cmp::max;
use std::convert::TryInto;

const DIMENSION: usize = 2;
const DIMENSION2: usize = DIMENSION * 2; // As we need 2 range checks for each dimension
const RADIUS: usize = 14;
const RANGE_BITS: usize = 6; // RANGE = 2^RANGE_BITS
const LOC_FUNC_COUNT: usize = 3;

pub struct SAPSIReceiver {
    n: usize,
    table_size: usize,
}

impl SAPSIReceiver {
    pub fn new(n: usize, table_size: usize) -> Self {
        SAPSIReceiver {
            n,
            table_size,
        }
    }

    pub fn receive<IO: CommunicationChannel>(&self, io: &mut IO, values: &[[u128; DIMENSION]], comm: &mut u64) {
        let mut processed_points: Vec<([u128; DIMENSION2], [u128; DIMENSION2])> = Vec::new();
        values.iter().for_each(|point| {
            let processed_point= preprocess_point(point);
            processed_points.extend_from_slice(processed_point.as_slice());
        });


        let mut simple_table = SimpleHash::<DIMENSION2>::new(self.table_size, 100, LOC_FUNC_COUNT);
        simple_table.generate_loc_funcs(LOC_FUNC_COUNT, Some([0u8; 16]));
        processed_points.iter().for_each(|(origin, transformed_point)| {
            let res: bool = simple_table.insert(origin, transformed_point);
            assert!(res, "Insertion failed");
        });

        // print out the simple table
        for index in 0..self.table_size {
            println!("Index: {}", index);
            let points = simple_table.query_table(index);
            points.iter().for_each(|(origin, transformed_point)| {
                println!("Origin: {:?}, Transformed Point: {:?}", origin, transformed_point);
            });
            println!("---------------------");
        }

        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();

        // Prepare OTs
        let depth: usize = RANGE_BITS;
        let mut sender_cot = BaseCot::new(0, false); // Receiver has role Sender in the OTs

        // Set up the receiver's precomputation phase
        sender_cot.cot_gen_pre(io, None, comm);

        // Original COT generation
        let size = depth + 1; // Number of COTs
        let times = self.table_size * DIMENSION * 2;
        // New COT generation using OTPre
        let mut sender_pre_ot = OTPre::<3>::new(size, times);
        sender_cot.cot_gen_preot(io, &mut sender_pre_ot, size * times, None, comm);

        // Sample random beta
        let mut beta = vec![[0u8; 16]; times];
        let mut key = vec![[0u8; 16]; times];
        let mut rng_seed = rand::thread_rng();
        for i in 0..times {
            rng_seed.fill(&mut beta[i]);
            rng_seed.fill(&mut key[i]);
        }

        // Generate IDCF
        let start = Instant::now();

        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();
        let mut idcf_sender = IDCFSender::new(depth, times);
        for index in 0..self.table_size {
            idcf_table.push(Vec::new());
            for dim in 0..DIMENSION2 {
                let mut idcf_sharing = vec![[0u8; 16]; 1 << (RANGE_BITS + 1)];
                let idcf_time = 2 * index * DIMENSION + dim;
                idcf_sender.compute(&mut idcf_sharing, key[idcf_time], beta[idcf_time], idcf_time);
                idcf_table[index].push(idcf_sharing);
            }

        }
            
        println!("Receiver computed IDCF in {:?}", start.elapsed());
        
        let start = Instant::now();
        idcf_sender.send(io, &mut sender_pre_ot, comm);
        println!("Receiver sent IDCF in {:?}", start.elapsed());

        for index in 0..self.table_size {
            for dim in 0..DIMENSION2 {
                idcf_sender.consistency_check(io, &idcf_table[index][dim], index * DIMENSION2 + dim);
            }
        }

        let mut hashes_set: Vec<HashSet<[u8; 32]>> = Vec::new();

        // Send the hash values
        let start = Instant::now();

        // Delta= 16, Mini universe 32 * 32
        // m * 2^DIMENSION * 3 * (6^DIMENSION)

        let mut max_bin_size = 0;
        let mut total_size = 0;

        for index in 0..self.table_size {
            let points_set = simple_table.query_table(index);
            points_set.iter().for_each(|(_origin, transformed_point)| {
                let mut hashes = HashSet::<[u8; 32]>::new();
                let prefixes = get_prefixes(transformed_point);
                prefixes.iter().for_each(|prefix| {
                    // Get the corresponding hash
                    let mut to_be_hashed = Vec::<u8>::new();
                    to_be_hashed.extend_from_slice(&index.to_le_bytes());
                    for i in DIMENSION2..0 {
                        to_be_hashed.extend_from_slice(&prefix[i].to_le_bytes());
                    }
                    for i in 0..DIMENSION2 {
                        to_be_hashed.extend_from_slice(idcf_table[index][i][prefix[i] as usize].as_slice());
                    }
                    let mut hasher= Blake2s256::new();
                    hasher.update(&to_be_hashed);
                    let mut hsh = [0u8; 32];
                    hsh.copy_from_slice(&hasher.finalize());
                    hashes.insert(hsh);
                });
                hashes_set.push(hashes);
            });
            total_size += points_set.len();
            max_bin_size = max(max_bin_size, points_set.len());
        }

        println!("Max bin size: {}", max_bin_size);
        println!("Total size: {}", total_size);

        println!("Receiver computed hashes in {:?}", start.elapsed());

        let mut hashes: Vec<Vec<[u8; 32]>> = Vec::new();
        for index in 0..self.table_size {
            let mut hash = Vec::<[u8; 32]>::new();
            let mut i = 0;
            hashes_set[index].iter().for_each(|h| {
                hash.push(*h);
            });
            hashes.push(hash);
        }

        for index in 0..self.table_size {
            *comm += io.send_block::<32>(&hashes[index]).expect("Failed to send intersection hash");
        }
    }
}

fn preprocess_point(point: &[u128; DIMENSION]) -> Vec<([u128; DIMENSION2], [u128; DIMENSION2])> {
    let mut result = Vec::<([u128; DIMENSION2], [u128; DIMENSION2])>::new();
    let mut grid_origin = [0u128; DIMENSION2];
    for i in 0..DIMENSION {
        grid_origin[2 * i] = (point[i] >> (RANGE_BITS - 1)) << (RANGE_BITS - 1);
        grid_origin[2 * i + 1] = grid_origin[2 * i];
    }

    for mask in 0..(1 << DIMENSION) {
        let mut universe_origin= grid_origin.clone();
        for i in 0..DIMENSION {
            if (mask >> i) & 1 == 1 {
                universe_origin[2 * i] -= (1 << (RANGE_BITS - 1));
                universe_origin[2 * i + 1] -= (1 << (RANGE_BITS - 1));
            }
        }
        let mut transformed_point = [0u128; DIMENSION2];
        for i in 0..DIMENSION {
            transformed_point[2 * i] = point[i] - universe_origin[2 * i];
            transformed_point[2 * i + 1] = (1 << RANGE_BITS) - 1 - (point[i] - universe_origin[2 * i + 1]);
        }

        result.push((universe_origin, transformed_point));
    }
    result
}

fn get_prefixes(point: &[u128; DIMENSION2]) -> Vec<[u128; DIMENSION2]> {
    let mut prefixes = vec![[0u128; DIMENSION2]];
    for i in 0..DIMENSION2 {
        let mut new_prefixes = Vec::<[u128; DIMENSION2]>::new();
        prefixes.iter().for_each(|prefix| {
            let mut new_prefix = prefix.clone();
            for j in 0..RANGE_BITS {
                new_prefix[i] |= ((point[i] >> j) & 1) << j;
                if j > 1 {
                    new_prefixes.push(new_prefix);
                }
            }
        });
        prefixes = new_prefixes;
    }
    prefixes
}