use std::time::Instant;
use std::vec;
use std::collections::HashSet;
use crate::cuckoo::{CuckooHash, SimpleHash}; 
use crate::idcf_receiver::IDCFReceiver;
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_network::comm_channel::CommunicationChannel;
use rand::prelude::*;
use blake2::{Blake2s256, Digest};

const DIMENSION: usize = 2;
const RADIUS_BITS: usize = 4;
const RANGE_BITS: usize = RADIUS_BITS + 2; // RANGE = 2^RANGE_BITS
const LOC_FUNC_COUNT: usize = 3;

// URGENT: Need to implement OPRF

pub struct SAPSISender {
    n: usize,
    table_size: usize,
    intersection: HashSet<[[u8; 16]; DIMENSION]>,
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
        let origins: Vec<[u128; DIMENSION]> = values.iter().map(|point| get_origin(point)).collect();
        let recentered_points: Vec<[u128; DIMENSION]> = values.iter().enumerate().map(|(i, point)| {
            let mut recentered_point = [0u128; DIMENSION];
            for j in 0..DIMENSION {
                recentered_point[j] = point[j] - origins[i][j];
            }
            recentered_point
        }).collect();

        let mut cuckoo_table = CuckooHash::<DIMENSION>::new(self.table_size, 20);
        cuckoo_table.generate_loc_funcs(LOC_FUNC_COUNT, Some([0u8; 16]));

        origins.iter().zip(recentered_points.iter()).for_each(|(origin, recentered_point)| {
            // println!("Inserting origin: {:?}", origin);
            let res: bool = cuckoo_table.insert(origin, recentered_point);
            assert!(res, "Insertion failed");
            // println!("Inserted point: {:?}", recentered_point);
        });


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
        let mut receiver_pre_ot = OTPre::<3>::new(size, times);
        receiver_cot.cot_gen_preot(io, &mut receiver_pre_ot, size * times, Some(&choice_bits), comm);

        let mut idcf_receiver = IDCFReceiver::new(depth, times);
        for index in 0..self.table_size {
            let (origin, recentered_point) = cuckoo_table.query_table(index);
            if recentered_point == [0u128; DIMENSION] {
                for dim in 0..DIMENSION {
                    idcf_receiver.set_alpha([0u8; 16], 2 * (index * DIMENSION + dim));
                    idcf_receiver.set_alpha([0u8; 16], 2 * (index * DIMENSION + dim) + 1);
                }
            } else {
                for dim in 0..DIMENSION {
                    let mut floor = recentered_point[dim] - (1 << RADIUS_BITS);
                    let mut ceil = recentered_point[dim] + (1 << RADIUS_BITS);

                    let alpha_floor = ((1 << RANGE_BITS) - floor).to_le_bytes();
                    idcf_receiver.set_alpha(alpha_floor, 2 * (index * DIMENSION + dim));
                    let alpha_ceil = (ceil + 1).to_le_bytes();
                    idcf_receiver.set_alpha(alpha_ceil, 2 * (index * DIMENSION + dim) + 1);
                }
            }
        }

        idcf_receiver.receive(io, &mut receiver_pre_ot, comm);

        let start = Instant::now();

        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();
        for index in 0..self.table_size {
            idcf_table.push(Vec::new());
            for dim in 0..DIMENSION {
                let mut idcf_sharing = vec![[0u8; 16]; 1 << (RANGE_BITS + 1)];
                idcf_receiver.compute(&mut idcf_sharing, index * DIMENSION + dim);
                idcf_table[index].push(idcf_sharing);
            }
        }

        println!("Sender computed IDCF in {:?}", start.elapsed());

        // Receive hashes from the receiver
        let mut hashes: Vec<Vec<[u8; 32]>> = Vec::new();
        for index in 0..self.table_size {
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
            let (_origin, recentered_point) = cuckoo_table.query_table(index);

            // Create set of indicating prefixes
            let mut all_set_decompose: Vec<Vec<([u8; 16], usize)>> = Vec::new();
            for dim in 0..DIMENSION {
                let alpha = recentered_point[dim].to_le_bytes();
                let set_decompose = set_decompose(&alpha);
                all_set_decompose.push(set_decompose);
            }
            let mut good_prefix = vec![([[0u8; 16]; DIMENSION], [0usize; DIMENSION]); 1];
            all_set_decompose.iter().enumerate().for_each(|(dim, set_decompose)| {
                let mut new_good_prefix = Vec::<([[u8; 16]; DIMENSION], [usize; DIMENSION])>::new();
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
                self.int_search(&idcf_table[index], index, &pref, &length, &hashes_set[index]);
            });
        }

        println!("Sender computed intersection in {:?}", start.elapsed());
    }

    pub fn int_search(&mut self, idcf_table: &Vec<Vec<[u8; 16]>>, index: usize, prefix: &[[u8; 16]; DIMENSION], length: &[usize; DIMENSION], hashes: &HashSet<[u8; 32]>) {
        // Get the corresponding hash
        let mut hasher = Blake2s256::new();
        hasher.update(&index.to_le_bytes());
        for i in 0..DIMENSION {
            hasher.update(prefix[i]);
        }
        for i in 0..DIMENSION {
            hasher.update(idcf_table[i][u128::from_le_bytes(prefix[i]) as usize]);
        }
        let mut hsh = [0u8; 32];
        hsh.copy_from_slice(&hasher.finalize());

        if !hashes.contains(&hsh) {
            return;
        }

        if *length == [RANGE_BITS; DIMENSION] {
            self.intersection.insert(*prefix);
        }

        for i in 0..DIMENSION {
            if length[i] < RANGE_BITS {
                for b in 0..2 {
                    let mut new_prefix = prefix.clone();
                    let mut new_length = length.clone();
                    new_prefix[i][length[i] / 8] |= b << (length[i] % 8);
                    new_length[i] += 1;
                    self.int_search(idcf_table, index, &new_prefix, &new_length, hashes);
                }
            }
        }
    }
}

fn get_origin(point: &[u128; DIMENSION]) -> [u128; DIMENSION] {
    let mut origin = [0u128; DIMENSION];
    for i in 0..DIMENSION {
        origin[i] = (point[i] >> (RADIUS_BITS + 1)) << (RADIUS_BITS + 1);
        if point[i] - origin[i] > (1 << RADIUS_BITS) {
            origin[i] += (1 << (RADIUS_BITS + 1));
        }
        origin[i] -= (1 << RADIUS_BITS);
    }
    origin
}

// This function returns both the prefixes and the length of these prefixes for search later
fn set_decompose(alpha: &[u8; 16]) -> Vec<([u8; 16], usize)> {
    let mut x = [0u8; 16];
    let mut res: Vec<([u8; 16], usize)> = Vec::new();
    for i in 0..RANGE_BITS {
        let bit = alpha[i / 8] >> (i % 8) & 1;
        x[i / 8] |= bit << (i % 8);
        if bit == 0 {
            continue;
        } else {
            x[i / 8] ^= 1 << (i % 8);
            res.push((x, i+1));
        }
    }
    res
}
