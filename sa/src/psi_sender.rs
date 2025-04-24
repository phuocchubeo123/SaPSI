use std::vec;
use crate::cuckoo::{CuckooHash, SimpleHash}; 
use crate::idcf_receiver::IDCFReceiver;
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_network::comm_channel::CommunicationChannel;
use rand::prelude::*;

const DIMENSION: usize = 2;
const RANGE_BITS: usize = 5; // RANGE = 2^RANGE_BITS
const LOC_FUNC_COUNT: usize = 3;

pub struct SAPSISender {
    n: usize,
    table_size: usize,
}

impl SAPSISender {
    pub fn new(n: usize, table_size: usize) -> Self {
        SAPSISender {
            n,
            table_size,
        }
    }

    pub fn send<IO: CommunicationChannel>(&self, io: &mut IO, values: &[[u128; DIMENSION]], comm: &mut u64) {
        let origins: Vec<[u128; DIMENSION]> = values.iter().map(|point| get_origin(point)).collect();
        let recentered_points: Vec<[u128; DIMENSION]> = values.iter().enumerate().map(|(i, point)| {
            let mut recentered_point = [0u128; DIMENSION];
            for j in 0..DIMENSION {
                recentered_point[j] = point[j] - origins[i][j];
            }
            recentered_point
        }).collect();

        let mut cuckoo_table = CuckooHash::<DIMENSION>::new(self.table_size, 10);
        cuckoo_table.generate_loc_funcs(LOC_FUNC_COUNT, None);

        origins.iter().zip(recentered_points.iter()).for_each(|(origin, recentered_point)| {
            let res: bool = cuckoo_table.insert(recentered_point, origin);
            assert!(res, "Insertion failed");
        });


        // Prepare OTs
        let depth: usize = RANGE_BITS;
        let mut receiver_cot = BaseCot::new(1, false);

        // Set up the receiver's precomputation phase
        receiver_cot.cot_gen_pre(io, None, comm);

        // Original COT generation
        let size = depth; // Number of COTs
        let times = self.table_size * DIMENSION;
        let mut choice_bits = vec![false; size * times];
        // Populate random choice bits
        for bit in &mut choice_bits {
            *bit = rand::random();
        }
        // New COT generation using OTPre
        let mut receiver_pre_ot = OTPre::<3>::new(size, times);
        receiver_cot.cot_gen_preot(io, &mut receiver_pre_ot, size * times, Some(&choice_bits), comm);

        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();
        for index in 0..self.table_size {
            idcf_table.push(Vec::new());
            let (origin, recentered_point) = cuckoo_table.query_table(index);
            for dim in 0..DIMENSION {
                let alpha = recentered_point[dim].to_le_bytes();
                println!("Alpha bits: {:?}", &alpha);
                let mut idcf_receiver = IDCFReceiver::new(depth);
                idcf_receiver.receive(io, &mut receiver_pre_ot, alpha, index * DIMENSION + dim, comm);

                let mut idcf_sharing = vec![[0u8; 16]; 1 << (RANGE_BITS + 1)];
                idcf_receiver.compute(&mut idcf_sharing);
                idcf_table[index].push(idcf_sharing);
            }
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