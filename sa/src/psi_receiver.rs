use crate::cuckoo::SimpleHash; 
use crate::idcf_sender::IDCFSender;
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_network::comm_channel::CommunicationChannel;
use rand::prelude::*;
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;

const DIMENSION: usize = 2;
const RANGE_BITS: usize = 5; // RANGE = 2^RANGE_BITS
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
        let origins: Vec<[u128; DIMENSION]> = values.iter().map(|point| get_origin(point)).collect();
        let recentered_points: Vec<[u128; DIMENSION]> = values.iter().enumerate().map(|(i, point)| {
            let mut recentered_point = [0u128; DIMENSION];
            for j in 0..DIMENSION {
                recentered_point[j] = point[j] - origins[i][j];
            }
            recentered_point
        }).collect();

        let mut simple_table = SimpleHash::<DIMENSION>::new(self.table_size, 10, LOC_FUNC_COUNT);
        origins.iter().zip(recentered_points.iter()).for_each(|(origin, recentered_point)| {
            let res: bool = simple_table.insert(origin, recentered_point);
            assert!(res, "Insertion failed");
        });

        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();

        // Prepare OTs
        let depth: usize = RANGE_BITS;
        let mut sender_cot = BaseCot::new(0, false); // Receiver has role Sender in the OTs

        // Set up the receiver's precomputation phase
        sender_cot.cot_gen_pre(io, None, comm);

        // Original COT generation
        let size = depth + 1; // Number of COTs
        let times = self.table_size * DIMENSION;
        // New COT generation using OTPre
        let mut sender_pre_ot = OTPre::<3>::new(size, times);
        sender_cot.cot_gen_preot(io, &mut sender_pre_ot, size * times, None, comm);

        // Sample random beta
        let mut beta = [0u8; 16];
        let mut key = [0u8; 16];
        let mut rng_seed = rand::thread_rng();
        rng_seed.fill(&mut beta);
        rng_seed.fill(&mut key);


        // Generate IDCF
        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();
        for index in 0..self.table_size {
            idcf_table.push(Vec::new());
            for dim in 0..DIMENSION {
                let mut idcf_sender = IDCFSender::new(depth);
                let mut idcf_sharing = vec![[0u8; 16]; 1 << (RANGE_BITS + 1)];
                idcf_sender.compute(&mut idcf_sharing, key, beta);
                idcf_sender.send(io, &mut sender_pre_ot, index * DIMENSION + dim, comm);
                idcf_table[index].push(idcf_sharing);
            }
        }

        let mut hashes_set: Vec<HashSet<[u8; 16]>> = Vec::new();

        // Send the hash values
        for index in 0..self.table_size {
            hashes_set.push(HashSet::<[u8; 16]>::new());
            let points_set = simple_table.query_table(index);
            points_set.iter().for_each(|(origin, recentered_point)| {
                let prefixes = get_prefixes(recentered_point);
                prefixes.iter().for_each(|prefix| {
                    // Get the corresponding hash
                    let mut hasher = Sha3_256::new();
                    hasher.update(&index.to_le_bytes());
                    for i in 0..DIMENSION {
                        hasher.update(prefix[i].to_le_bytes());
                    }
                    for i in 0..DIMENSION {
                        hasher.update(idcf_table[index][i][prefix[i] as usize]);
                    }
                    let mut hsh = [0u8; 16];
                    hsh.copy_from_slice(&hasher.finalize()[..16]);
                    hashes_set[index].insert(hsh);
                });
            });
        }

        let mut hashes: Vec<Vec<[u8; 16]>> = Vec::new();
        for index in 0..self.table_size {
            let mut hash = Vec::<[u8; 16]>::new();
            let mut i = 0;
            hashes_set[index].iter().for_each(|h| {
                hash.push(*h);
            });
            hashes.push(hash);
        }

        for index in 0..self.table_size {
            *comm += io.send_block::<16>(&hashes[index]).expect("Failed to send intersection hash");
        }
    }
}

fn get_origin(point: &[u128; DIMENSION]) -> [u128; DIMENSION] {
    let mut origin = [0u128; DIMENSION];
    for i in 0..DIMENSION {
        origin[i] = point[i];
        origin[i] = (origin[i] >> RANGE_BITS) << RANGE_BITS;
    }
    origin
}

fn get_prefixes(point: &[u128; DIMENSION]) -> Vec<[u128; DIMENSION]> {
    let mut prefixes = vec![[0u128; DIMENSION]];
    for i in 0..DIMENSION {
        let mut new_prefixes = Vec::<[u128; DIMENSION]>::new();
        prefixes.iter().for_each(|prefix| {
            let mut new_prefix = prefix.clone();
            for j in 0..RANGE_BITS {
                new_prefix[i] |= ((point[i] >> j) & 1) << j;
                new_prefixes.push(new_prefix);
            }
        });
        prefixes = new_prefixes;
    }
    prefixes
}