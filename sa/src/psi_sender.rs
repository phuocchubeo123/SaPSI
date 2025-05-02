use crate::config::*;
use crate::cuckoo::{CuckooHash, SimpleHash}; 
use crate::idcf_receiver::IDCFReceiver;
use std::time::Instant;
use std::vec;
use std::collections::HashSet;
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_network::comm_channel::CommunicationChannel;
use rand::prelude::*;
use blake2::{Blake2s256, Digest};

// URGENT: Need to implement OPRF

pub struct SAPSISender {
    n: usize,
    table_size: usize,
    intersection: HashSet<[u128; DIMENSION2]>,
}

impl SAPSISender {
    pub fn new(n: usize, table_size: usize) -> Self {
        SAPSISender {
            n,
            table_size,
            intersection: HashSet::new(),
        }
    }

    pub fn send<IO: CommunicationChannel>(&mut self, io: &mut IO, values: &[[u128; DIMENSION]], comm: &mut u64) {
        // All (origin, recentered_point) pairs
        let processed_points: Vec<([u128; DIMENSION2], [u128; DIMENSION2])> = values.iter()
            .map(|point| preprocess_point(point))
            .collect();

        // Prepare cuckoo hash table
        let mut cuckoo_table = CuckooHash::<DIMENSION2>::new(self.table_size, 100);
        cuckoo_table.generate_loc_funcs(LOC_FUNC_COUNT, Some([0u8; 16]));

        processed_points.iter().for_each( |(origin, recentered_point)| {
            let res: bool = cuckoo_table.insert(origin, recentered_point);
            assert!(res, "Insertion failed");
        });

        // print out the cuckoo table
        // for index in 0..self.table_size {
        //     let (origin, compare_point) = cuckoo_table.query_table(index);
        //     println!("Index: {}", index);
        //     println!("Origin: {:?}, Compare Point: {:?}", origin, compare_point);
        //     println!("---------------------");
        // }

        // Prepare OTs
        let depth: usize = RANGE_BITS;
        let mut receiver_cot = BaseCot::new(1, false);

        // Set up the receiver's precomputation phase
        receiver_cot.cot_gen_pre(io, None, comm);

        // Original COT generation
        let size = depth + 1; // Number of COTs
        let times = self.table_size * DIMENSION * 2;
        println!("Times: {}", times);
        let mut choice_bits = vec![false; size * times];
        // Populate random choice bits
        for bit in &mut choice_bits {
            *bit = rand::random();
        }
        // New COT generation using OTPre
        let mut receiver_pre_ot = OTPre::<3>::new(size * times, 1);
        receiver_cot.cot_gen_preot(io, &mut receiver_pre_ot, size * times, Some(&choice_bits), comm);

        let mut idcf_receiver = IDCFReceiver::new(depth, times);
        for index in 0..self.table_size {
            let (_universe_origin, compare_point) = cuckoo_table.query_table(index);
            if compare_point == [0u128; DIMENSION2] {
                for dim in 0..DIMENSION2 {
                    idcf_receiver.set_alpha([0u8; 16], 2 * index * DIMENSION + dim);
                }
            } else {
                for dim in 0..DIMENSION2 {
                    let alpha = compare_point[dim].to_le_bytes();
                    idcf_receiver.set_alpha(alpha, 2 * index * DIMENSION + dim);
                }
            }
        }

        idcf_receiver.receive(io, &mut receiver_pre_ot, comm);

        let start = Instant::now();

        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();
        for index in 0..self.table_size {
            idcf_table.push(Vec::new());
            for dim in 0..DIMENSION2 {
                let mut idcf_sharing = vec![[0u8; 16]; 1 << (RANGE_BITS + 1)];
                idcf_receiver.compute(&mut idcf_sharing, 2 * index * DIMENSION + dim);
                idcf_table[index].push(idcf_sharing);
            }
        }

        // for index in 0..self.table_size {
        //     for dim in 0..DIMENSION2 { 
        //         idcf_receiver.consistency_check(io, &idcf_table[index][dim], index * DIMENSION2 + dim);
        //     }
        // }
        println!("Sender computed IDCF in {:?}", start.elapsed());

        // Receive hashes from the receiver
        let mut hashes: Vec<Vec<[u8; 32]>> = Vec::new();
        for _index in 0..self.table_size {
            let hash = io.receive_block::<32>().expect("Failed to receive intersection hash from receiver");
            hashes.push(hash);
        }

        let mut hashes_set: Vec<HashSet<[u8; 32]>> = Vec::new();
        for index in 0..self.table_size {
            let mut hash_set = HashSet::new();
            hashes[index].iter().for_each(|h| {
                hash_set.insert(*h);
            });
            hashes_set.push(hash_set);
        }

        let start = Instant::now();
        // Now start doing PSI in each bin
        for index in 0..self.table_size {
            let (_origin, compare_point) = cuckoo_table.query_table(index);

            // println!("Index: {}", index);
            // println!("Compare Point: {:?}", compare_point);

            // Create set of indicating prefixes
            let mut all_set_decompose: Vec<Vec<(u128, usize)>> = Vec::new();
            for dim in 0..DIMENSION2 {
                let set_decompose = set_decompose(compare_point[dim]);
                all_set_decompose.push(set_decompose);
            }
            let mut good_prefix = vec![([0u128; DIMENSION2], [0usize; DIMENSION2]); 1];
            all_set_decompose.iter().enumerate().for_each(|(dim, set_decompose)| {
                let mut new_good_prefix = Vec::<([u128; DIMENSION2], [usize; DIMENSION2])>::new();
                good_prefix.iter().for_each(|(pref, length_tuple)| {
                    let mut new_pref = *pref;
                    let mut new_length_tuple = *length_tuple;
                    set_decompose.iter().for_each(|(decompose, length)| {
                        new_pref[dim] = *decompose;
                        new_length_tuple[dim] = *length;
                        new_good_prefix.push((new_pref, new_length_tuple));
                    });
                });
                good_prefix = new_good_prefix;
            });

            // Now start DFS
            good_prefix.iter().for_each(|(pref, length)| {
                // println!("Prefix: {:?}, Length: {:?}", pref, length);
                self.int_search(&idcf_table[index], index, &pref, &length, &hashes_set[index]);
            });

            // println!();
        }

        // for dim in 0..DIMENSION2 {
        //     for layer in 1..(depth + 1) {
        //         println!("Layer {}", layer);
        //         for x in 0..(1 << layer) {
        //             println!("IDCF: {:?}", idcf_table[27][dim][(1 << layer) - 1 + x]);
        //         }
        //     }
        // }


        println!("Sender computed intersection in {:?}", start.elapsed());
        println!("Intersection size: {}", self.intersection.len());
    }

    pub fn int_search(&mut self, idcf_table: &Vec<Vec<[u8; 16]>>, index: usize, prefix: &[u128; DIMENSION2], length: &[usize; DIMENSION2], hashes: &HashSet<[u8; 32]>) {
        for i in 0..DIMENSION2 {
            if length[i] < PREF_CUT {
                for b in 0..2 {
                    let mut new_prefix = prefix.clone();
                    let mut new_length = length.clone();
                    new_prefix[i] = new_prefix[i] * 2 + b;
                    new_length[i] += 1;
                    self.int_search(idcf_table, index, &new_prefix, &new_length, hashes);
                }
                return;
            }
        }
        // Get the corresponding hash
        let mut to_be_hashed = Vec::<u8>::new();
        to_be_hashed.extend_from_slice(&index.to_le_bytes());
        for i in 0..DIMENSION2 {
            to_be_hashed.extend_from_slice(&prefix[i].to_le_bytes());
        }
        for i in 0..DIMENSION2 {
            to_be_hashed.extend_from_slice(idcf_table[i][(1 << length[i]) - 1 + prefix[i] as usize].as_slice());
        }
        let mut hasher = Blake2s256::new();
        hasher.update(&to_be_hashed);
        let mut hsh = [0u8; 32];
        hsh.copy_from_slice(&hasher.finalize());

        // if index == 27 {
        //     println!("Prefix: {:?}, Length: {:?}", prefix, length);
        //     println!("To be hashed: {:?}", to_be_hashed);
        //     println!("Hash: {:?}", hsh);
        // }

        // If the critical prefix hash is not in the hash set, prune
        if !hashes.contains(&hsh) {
            return;
        }

        // println!("Found a match: {:?}, {:?}", prefix, length);

        if *length == [RANGE_BITS; DIMENSION2] {
            self.intersection.insert(*prefix);
        }

        for i in 0..DIMENSION2 {
            if length[i] < RANGE_BITS {
                for b in 0..2 {
                    let mut new_prefix = prefix.clone();
                    let mut new_length = length.clone();
                    new_prefix[i] = new_prefix[i] * 2 + b;
                    new_length[i] += 1;
                    self.int_search(idcf_table, index, &new_prefix, &new_length, hashes);
                }
                return;
            }
        }
    }
}

fn preprocess_point(point: &[u128; DIMENSION]) -> ([u128; DIMENSION2], [u128; DIMENSION2]) {
    let mut grid_origin= [0u128; DIMENSION2];
    let mut universe_origin = [0u128; DIMENSION2];
    for i in 0..DIMENSION {
        grid_origin[2 * i] = (point[i] >> (RANGE_BITS - 1)) << (RANGE_BITS - 1);
        if point[i] + (RADIUS as u128) + 1 < grid_origin[2 * i] + (1 << (RANGE_BITS - 1)) {
            universe_origin[2 * i] = grid_origin[2 * i] - (1 << (RANGE_BITS - 1));
        } else {
            universe_origin[2 * i] = grid_origin[2 * i];
        }
        universe_origin[2 * i + 1] = universe_origin[2 * i];
    }

    let mut compare_point = [0u128; DIMENSION2];
    for i in 0..DIMENSION {
        compare_point[2 * i] = point[i] - universe_origin[2 * i] + (RADIUS as u128) + 1;
        compare_point[2 * i + 1] = (1 << RANGE_BITS) - (point[i] - universe_origin[2 * i + 1] - (RADIUS as u128));
    }

    (universe_origin, compare_point)
}

// This function returns both the prefixes and the length of these prefixes for search later
fn set_decompose(point: u128) -> Vec<(u128, usize)> {
    let mut x = point;
    let mut res: Vec<(u128, usize)> = Vec::new();
    for i in (0..RANGE_BITS).rev() {
        if (x & 1) == 1 {
            res.push((x ^ 1, i + 1));
        } 
        x >>= 1;
    }
    // println!("Point: {}, Decomposed: {:?}", point, res);
    res
}
