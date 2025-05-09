use crate::vole_triple_f2k::VoleTripleF2k;
use psi_okvs::okvs_f2k::RbOkvsF2k;

pub struct OprfSenderF2k {
    n: usize,
    vole_sender: VoleTripleF2k,
    b: Vec<u128>,
    K: Vec<u128>,
    delta: u128,
    committed: bool,
    okvs: RbOkvsF2k,
    w: u128, 
    outputs_byte: Vec<[u8; 32]>,
}
