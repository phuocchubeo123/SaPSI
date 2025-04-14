use crate::cuckoo::CuckooHash;
use psi_network::comm_channel::CommunicationChannel;

const DIMENSION: usize = 2;
const RANGE_BITS: usize = 5; // RANGE = 2^RANGE_BITS

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

    pub fn send<IO: CommunicationChannel>(&self, io: &mut IO, values: &[[u128; DIMENSION]]) {
        let origins: Vec<[u128; DIMENSION]> = values.iter().map(|point| get_origin(point)).collect();
        let cuckoo_table = CuckooHash::<DIMENSION>::new(self.table_size, 10);
        
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