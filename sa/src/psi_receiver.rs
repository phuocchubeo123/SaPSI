use crate::cuckoo::SimpleHash; 
use crate::idcf_sender::IDCFSender;
use psi_ot::base_cot::BaseCot;
use psi_ot::pre_ot::OTPre;
use psi_network::comm_channel::CommunicationChannel;
use rand::prelude::*;

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
            let res: bool = simple_table.insert(recentered_point, origin);
            assert!(res, "Insertion failed");
        });

        let mut idcf_table = Vec::<Vec<Vec<[u8; 16]>>>::new();

        // Prepare OTs
        let depth: usize = RANGE_BITS;
        let mut sender_cot = BaseCot::new(0, false); // Receiver has role Sender in the OTs

        // Set up the receiver's precomputation phase
        sender_cot.cot_gen_pre(io, None, comm);

        // Original COT generation
        let size = depth; // Number of COTs
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