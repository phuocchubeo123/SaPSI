use crate::vole_triple_f2k::{PrimalLPNParameterF2k, VoleTripleF2k};
use psi_network::comm_channel::CommunicationChannel;
use psi_okvs::okvs_f2k::RbOkvsF2k;
use psi_aes::prg::PRG;
use psi_utils::gf128::gf128mul;
use blake3;

pub struct OprfSenderF2k {
    n: usize,
    vole_sender: VoleTripleF2k,
    b: Vec<u128>,
    K: Vec<u128>,
    delta: u128,
    okvs: RbOkvsF2k,
    w: u128, 
    outputs_byte: Vec<[u8; 32]>,
}

impl OprfSenderF2k {
    pub fn new<IO: CommunicationChannel>(io: &mut IO, n: usize, param: PrimalLPNParameterF2k, comm: &mut u64) -> Self {
        // Setup delta
        let mut prg = PRG::new(None, 0);
        let mut delta_bytes= [[0u8; 16]; 1];
        prg.random_16byte_block(&mut delta_bytes);
        let delta = u128::from_le_bytes(delta_bytes[0]);

        // Receive OKVS seed from the receiver
        let r = io.receive_block::<16>().expect("Failed to receive okvs seed");
        let r1 = r[0];
        let r2 = r[1];
        let okvs = RbOkvsF2k::new(n, &r1, &r2);

        let mut vole_triple = VoleTripleF2k::new(0, true, io, param, comm);
        vole_triple.setup_sender(io, delta, comm);
        vole_triple.extend_initialization();

        OprfSenderF2k {
            n,
            vole_sender: vole_triple,
            b: vec![0; n],
            K: vec![0; 2*n],
            delta,
            okvs,
            w: 0,
            outputs_byte: vec![[0u8; 32]; n],
        }
    }

    pub fn send<IO: CommunicationChannel>(&mut self, io: &mut IO, values: &[u128], comm: &mut u64) {
        // Creating ws and send H(ws) to the receiver
        let mut prg = PRG::new(None, 0);
        let mut ws_bytes= [[0u8; 16]; 1];
        prg.random_16byte_block(&mut ws_bytes);
        let ws = u128::from_le_bytes(ws_bytes[0]);
        let mut hash_buf = 0u128;
        // hash ws
        let hash = blake3::hash(&ws.to_le_bytes());
        let mut ws_hash = [0u8; 32];
        ws_hash.copy_from_slice(hash.as_bytes());
        *comm += io.send_block::<32>(&[ws_hash]).expect("Failed to send hash");

        // Running Vole
        // c = b + a * delta
        let mut z = vec![0u128; self.okvs.columns];
        self.vole_sender.extend(io, &mut self.b, &mut z, self.okvs.columns, comm); 

        // Get w = ws + wr;
        let wr_bytes = io.receive_block::<16>().expect("Failed to receive wr");
        let wr = u128::from_le_bytes(wr_bytes[0]);
        self.w = ws ^ wr;
        let ws_bytes = ws.to_le_bytes();
        *comm += io.send_block::<16>(&[ws_bytes]).expect("Failed to send ws");

        // Receive A = P + a from the receiver and get K = b + A * delta
        let A_bytes = io.receive_block::<16>().expect("Failed to receive A");
        let A = A_bytes.iter().map(|&x| u128::from_le_bytes(x)).collect::<Vec<u128>>();
        let mut K = vec![0u128; 2*self.n];
        K.iter_mut().enumerate().for_each(|(i, Ki)| {
            *Ki = self.b[i] ^ gf128mul(A[i], self.delta);
        });
        self.K = K;

        let mut o = self.okvs.decode(&self.K, values);

        o.iter_mut().enumerate().for_each(|(i, oi)| {
            let hash = blake3::hash(&values[i].to_le_bytes());
            let mut val_hash_bytes = [0u8; 16];
            val_hash_bytes.copy_from_slice(&hash.as_bytes()[0..16]);
            let val_hash = u128::from_le_bytes(val_hash_bytes);
            *oi = *oi ^ gf128mul(self.delta, val_hash) ^ self.w;
        });

        self.outputs_byte = vec![[0u8; 32]; self.n];
        self.outputs_byte.iter_mut().enumerate().for_each(|(i, outputs_i)| {
            let mut to_be_hashed = Vec::<u8>::new();
            to_be_hashed.extend_from_slice(&values[i].to_le_bytes());
            to_be_hashed.extend_from_slice(&o[i].to_le_bytes());
            let hash = blake3::hash(&to_be_hashed);
            let mut hash_bytes = [0u8; 32];
            hash_bytes.copy_from_slice(hash.as_bytes());
            *outputs_i = hash_bytes;
        });

        *comm += io.send_block::<32>(&self.outputs_byte).expect("Failed to send output hashes");
    }
}