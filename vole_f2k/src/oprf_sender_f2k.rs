use crate::vole_triple_f2k::{PrimalLPNParameterF2k, VoleTripleF2k};
use psi_network::tcp_channel::TcpChannel;
use psi_okvs::okvs_f2k::RbOkvsF2k;
use psi_aes::prg::PRG;
use psi_utils::gf128::gf128mul;
use blake3;
use std::collections::HashMap;

pub struct OprfSenderF2k<const KEY_DIM: usize> {
    vole_sender: VoleTripleF2k,
    b: Vec<u128>,
    big_k: Vec<u128>,
    delta: u128,
    okvs: RbOkvsF2k<KEY_DIM>,
    w: u128, 
    outputs: HashMap<[u128; KEY_DIM], [u8; 32]>,
}

impl<const KEY_DIM: usize> OprfSenderF2k<KEY_DIM> {
    pub fn new(io: &mut TcpChannel, n: usize, param: PrimalLPNParameterF2k) -> Self {
        // Setup delta
        let mut prg = PRG::new(None, 0);
        let mut delta_bytes= [[0u8; 16]; 1];
        prg.random_16byte_block(&mut delta_bytes);
        let delta = u128::from_le_bytes(delta_bytes[0]);

        // Receive OKVS seed from the receiver
        let r = io.receive_block::<16>().expect("Failed to receive okvs seed");
        let r1 = r[0];
        let r2 = r[1];
        let okvs = RbOkvsF2k::<KEY_DIM>::new(n, &r1, &r2);

        let mut vole_triple = VoleTripleF2k::new(0, true, io, param);
        vole_triple.setup_sender(io, delta);
        vole_triple.extend_initialization();

        OprfSenderF2k {
            vole_sender: vole_triple,
            b: vec![0; okvs.columns],
            big_k: vec![0; okvs.columns],
            delta,
            okvs,
            w: 0,
            outputs: HashMap::new(),
        }
    }

    pub fn send(&mut self, io: &mut TcpChannel, values: &[[u128; KEY_DIM]]) {
        // Creating ws and send H(ws) to the receiver
        let mut prg = PRG::new(None, 0);
        let mut ws_bytes= [[0u8; 16]; 1];
        prg.random_16byte_block(&mut ws_bytes);
        let ws = u128::from_le_bytes(ws_bytes[0]);
        // hash ws
        let hash = blake3::hash(&ws.to_le_bytes());
        let mut ws_hash = [0u8; 32];
        ws_hash.copy_from_slice(hash.as_bytes());
        io.send_block::<32>(&[ws_hash]).expect("Failed to send hash");

        // Running Vole
        // c = b + a * delta
        println!("Number of vole: {}", self.okvs.columns);
        let mut z = vec![0u128; self.okvs.columns];
        self.vole_sender.extend(io, &mut self.b, &mut z, self.okvs.columns); 

        println!("Done vole");

        // Get w = ws + wr;
        let wr_bytes = io.receive_block::<16>().expect("Failed to receive wr");
        let wr = u128::from_le_bytes(wr_bytes[0]);
        self.w = ws ^ wr;
        let ws_bytes = ws.to_le_bytes();
        io.send_block::<16>(&[ws_bytes]).expect("Failed to send ws");

        // Receive A = P + a from the receiver and get K = b + A * delta
        let big_a_bytes = io.receive_block::<16>().expect("Failed to receive A");
        let big_a = big_a_bytes.iter().map(|&x| u128::from_le_bytes(x)).collect::<Vec<u128>>();
        let mut big_k = vec![0u128; self.okvs.columns];
        big_k.iter_mut().enumerate().for_each(|(i, big_ki)| {
            *big_ki = self.b[i] ^ gf128mul(big_a[i], self.delta);
        });
        self.big_k = big_k;

        let mut o = self.okvs.decode(&self.big_k, values);

        o.iter_mut().enumerate().for_each(|(i, oi)| {
            let mut hash = blake3::Hasher::new();
            values[i].iter().for_each(|&x| {
                hash.update(&x.to_le_bytes());
            });
            let mut val_hash_bytes = [0u8; 16];
            val_hash_bytes.copy_from_slice(&hash.finalize().as_bytes()[0..16]);
            let val_hash = u128::from_le_bytes(val_hash_bytes);
            *oi = *oi ^ gf128mul(self.delta, val_hash) ^ self.w;
        });

        let mut outputs_byte = vec![[0u8; 32]; values.len()];
        outputs_byte.iter_mut().enumerate().for_each(|(i, outputs_i)| {
            let mut to_be_hashed = Vec::<u8>::new();
            values[i].iter().for_each(|&x| {
                to_be_hashed.extend_from_slice(&x.to_le_bytes());
            });
            to_be_hashed.extend_from_slice(&o[i].to_le_bytes());
            let hash = blake3::hash(&to_be_hashed);
            let mut hash_bytes = [0u8; 32];
            hash_bytes.copy_from_slice(hash.as_bytes());
            *outputs_i = hash_bytes;
        });

        values.iter().zip(outputs_byte.iter()).for_each(|(x, o)| {
            self.outputs.insert(*x, *o);
        });
    }

    pub fn get_output(&self, x: &[u128; KEY_DIM]) -> Option<[u8; 32]> {
        self.outputs.get(x).copied()
    }
}