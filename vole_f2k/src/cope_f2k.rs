use psi_ot::otco::OTCO;
use psi_aes::prg::{F, PRG};
use psi_utils::gf128::gf128mul;
use psi_network::comm_channel::CommunicationChannel;

const NUM_BITS: usize = 128;

pub struct CopeF2k {
    party: u8,
    delta: Option<u128>,
    delta_bool: [bool; NUM_BITS],
    prg_g0: Option<Vec<PRG>>,
    prg_g1: Option<Vec<PRG>>,
    mask: u128,
    powers_of_two: Vec<u128>,
}

impl CopeF2k {
    /// Create a new COPE instance.
    pub fn new(party: u8) -> Self {
        Self {
            party,
            delta: None,
            delta_bool: [false; NUM_BITS],
            prg_g0: None,
            prg_g1: None,
            mask: u128::MAX,
            powers_of_two: vec![], // Initialize empty, will be filled in `initialize_*`
        }
    }

    /// Precompute powers of two in the field
    fn precompute_powers_of_two(&mut self) {
        let mut powers = vec![1u128; NUM_BITS];
        let base = 2u128;
        for i in 1..NUM_BITS {
            powers[i] = gf128mul(powers[i-1], base);
        }
        self.powers_of_two = powers;
    }

    pub fn initialize_sender<IO: CommunicationChannel>(&mut self, io: &mut IO, delta: u128, comm: &mut u64) {
        self.delta = Some(delta);
        self.delta_bool = delta_to_bool(delta);
        self.precompute_powers_of_two(); // Precompute powers of two

        // Prepare keys using OTCO
        let mut k = Vec::new();
        let mut otco = OTCO::new();
        otco.recv(io, &self.delta_bool, &mut k, comm);

        // Initialize PRGs
        self.prg_g0 = Some(
            k.iter()
                .enumerate()
                .map(|(i, key)| {
                    let mut prg = PRG::new(Some(key), (i + (self.delta_bool[i] as usize) * NUM_BITS) as u64);
                    prg
                })
                .collect(),
        );

        assert_eq!(k.len(), NUM_BITS, "Mismatch in key length during initialization");
        assert_eq!(self.prg_g0.as_ref().unwrap().len(), NUM_BITS, "Mismatch in prg_g0 length after initialization");
    }

    pub fn initialize_receiver<IO: CommunicationChannel>(&mut self, io: &mut IO, comm: &mut u64) {
        self.precompute_powers_of_two(); // Precompute powers of two

        let mut k0 = vec![[0u8; 16]; NUM_BITS];
        let mut k1 = vec![[0u8; 16]; NUM_BITS];

        // Generate random keys
        let mut key_prg = PRG::new(None, 0);
        key_prg.random_16byte_block(&mut k0);
        key_prg.random_16byte_block(&mut k1);

        // Use OTCO to send keys
        let mut otco = OTCO::new();
        otco.send(io, &k0, &k1, comm);

        // Initialize PRGs
        self.prg_g0 = Some(
            k0.iter()
                .enumerate()
                .map(|(i, key)| {
                    let prg = PRG::new(Some(key), i as u64);
                    prg
                })
                .collect(),
        );
        self.prg_g1 = Some(
            k1.iter()
                .enumerate()
                .map(|(i, key)| {
                    let prg = PRG::new(Some(key), (i + NUM_BITS) as u64);
                    prg
                })
                .collect(),
        );
    }

    pub fn extend_sender<IO: CommunicationChannel>(&mut self, io: &mut IO, comm: &mut u64) -> u128 {
        let mut w = vec![0u128; NUM_BITS];

        if let Some(prgs) = &mut self.prg_g0 {
            let mut w_bytes = vec![[0u8; 16]; NUM_BITS];

            for (i, prg) in prgs.iter_mut().enumerate() {
                prg.random_16byte_block(&mut w_bytes[i..i+1]);
            }
            for i in 0..NUM_BITS {
                w[i] = u128::from_le_bytes(w_bytes[i]);
            }
        }

        // Receive v from the receiver
        let mut v_bytes = io.receive_block::<16>().expect("Failed to receive v");
        let mut v = v_bytes
            .iter()
            .map(|&v_byte| u128::from_le_bytes(v_byte))
            .collect::<Vec<u128>>();

        // Adjust v based on delta_bool
        for i in 0..NUM_BITS {
            if self.delta_bool[i] {
                v[i] = w[i] ^ v[i];
            } else {
                v[i] = w[i];
            }
        }

        // Aggregate v into a single field element
        self.prm2pr(&v)
    }

    pub fn extend_sender_batch<IO: CommunicationChannel>(&mut self, io: &mut IO, ret: &mut [u128], size: usize, comm: &mut u64) {
        // Generate ret_recv = ret_send + delta * u_recv

        let mut w = vec![vec![0u128; size]; NUM_BITS];
        let mut v = vec![vec![0u128; size]; NUM_BITS];

        // Generate random w values for the batch
        if let Some(prgs) = &mut self.prg_g0 {
            let mut w_bytes = vec![vec![[0u8; 16]; size]; NUM_BITS];
            for (i, prg) in prgs.iter_mut().enumerate() {
                prg.random_16byte_block(&mut w_bytes[i]);

                for j in 0..size {
                    w[i][j] = u128::from_le_bytes(w_bytes[i][j]);
                }
            }
        }

        // Receive v values from the receiver
        let received_data = io.receive_block::<16>().expect("Failed to receive v");
        for i in 0..NUM_BITS {
            for j in 0..size {
                v[i][j] = u128::from_le_bytes(received_data[i * size + j]);
            }
        }

        // Adjust v values based on delta_bool
        for i in 0..NUM_BITS {
            for j in 0..size {
                if self.delta_bool[i] {
                    v[i][j] = w[i][j] ^ v[i][j];
                } else {
                    v[i][j] = w[i][j];
                }
            }
        }

        // Aggregate batch results into ret
        self.prm2pr_batch(ret, &v);
    }

    pub fn extend_receiver<IO: CommunicationChannel>(&mut self, io: &mut IO, u: u128, comm: &mut u64) -> u128 {
        let mut w0 = vec![0u128; NUM_BITS];
        let mut w1 = vec![0u128; NUM_BITS];
        let mut tau = vec![0u128; NUM_BITS];

        // Generate random w0 and w1 values
        if let (Some(prgs_g0), Some(prgs_g1)) = (&mut self.prg_g0, &mut self.prg_g1) {
            let mut w0_bytes = vec![[0u8; 16]; NUM_BITS];
            let mut w1_bytes = vec![[0u8; 16]; NUM_BITS];
            for i in 0..NUM_BITS {
                prgs_g0[i].random_16byte_block(&mut w0_bytes[i..i+1]);
                prgs_g1[i].random_16byte_block(&mut w1_bytes[i..i+1]);
            }

            for i in 0..NUM_BITS {
                w0[i] = u128::from_le_bytes(w0_bytes[i]);
                w1[i] = u128::from_le_bytes(w1_bytes[i]);
                w1[i] = w1[i] ^ u;
                tau[i] = w0[i] ^ w1[i];
            }
        }

        // Send tau to the sender
        let tau_bytes: Vec<[u8; 16]> = tau
            .iter()
            .map(|&tau_val| tau_val.to_le_bytes())
            .collect();
        *comm += io.send_block::<16>(&tau_bytes).expect("Failed to send tau");

        // Aggregate w0 into a single field element
        self.prm2pr(&w0)
    }

    pub fn extend_receiver_batch<IO: CommunicationChannel>(&mut self, io: &mut IO, ret: &mut [u128], u: &[u128], size: usize, comm: &mut u64) {
        // Generate ret_recv = ret_send + delta * u_recv

        let mut w0 = vec![vec![0u128; size]; NUM_BITS];
        let mut w1 = vec![vec![0u128; size]; NUM_BITS];
        let mut tau = vec![vec![0u128; size]; NUM_BITS];

        // Generate random w0 and w1 values
        if let (Some(prgs_g0), Some(prgs_g1)) = (&mut self.prg_g0, &mut self.prg_g1) {
            let mut w0_bytes = vec![vec![[0u8; 16]; size]; NUM_BITS];
            let mut w1_bytes = vec![vec![[0u8; 16]; size]; NUM_BITS];
            for i in 0..NUM_BITS {
                prgs_g0[i].random_16byte_block(&mut w0_bytes[i]);
                prgs_g1[i].random_16byte_block(&mut w1_bytes[i]);
            }
            for i in 0..NUM_BITS {
                for j in 0..size {
                    w0[i][j] = u128::from_le_bytes(w0_bytes[i][j]);
                    w1[i][j] = u128::from_le_bytes(w1_bytes[i][j]);
                    w1[i][j] = w1[i][j] ^ u[j];
                    tau[i][j] = w0[i][j] ^ w1[i][j];
                }
            }
        }

        // Send tau to the sender
        let tau_flat: Vec<u128> = tau.iter().flat_map(|row| row.iter().cloned()).collect();
        let tau_flat_bytes: Vec<[u8; 16]> = tau_flat
            .iter()
            .map(|&tau_val| tau_val.to_le_bytes())
            .collect();
        *comm += io.send_block::<16>(&tau_flat_bytes).expect("Failed to send tau");
        io.flush();

        // Aggregate w0 batch results into ret
        self.prm2pr_batch(ret, &w0);
    }

    /// Aggregates a vector of field elements into a single field element using precomputed powers of two.
    fn prm2pr(&self, elements: &[u128]) -> u128 {
        elements
            .iter()
            .zip(&self.powers_of_two)
            .fold(0u128, |acc, (e, power)| acc ^ gf128mul(*e, *power))
    }

    /// Aggregates a batch of vectors of field elements into a result array using precomputed powers of two.
    fn prm2pr_batch(&self, ret: &mut [u128], elements: &[Vec<u128>]) {
        for (j, result) in ret.iter_mut().enumerate() {
            *result = elements.iter().zip(&self.powers_of_two).fold(0u128, |acc, (row, power)| {
                acc ^ gf128mul(row[j], *power)
            });
        }
    }

    // Debug
    pub fn check_triple<IO: CommunicationChannel>(&mut self, io: &mut IO, a: &[u128], b: &[u128], sz: usize) {
        if self.party == 0 {
            // Sender's role
            let a_bytes = a.iter().map(|&x| x.to_le_bytes()).collect::<Vec<_>>();
            let b_bytes = b.iter().map(|&x| x.to_le_bytes()).collect::<Vec<_>>();
            io.send_block::<16>(&a_bytes).expect("Failed to send `a` in check_triple");
            io.send_block::<16>(&b_bytes).expect("Failed to send `b` in check_triple");
        } else {
            // Receiver's role
            let delta_bytes = io.receive_block::<16>().expect("Failed to receive `delta` in check_triple")[0];
            let c_bytes = io.receive_block::<16>().expect("Failed to receive `c` in check_triple");
            let delta = u128::from_le_bytes(delta_bytes);
            let c = c_bytes
                .iter()
                .map(|&c_byte| u128::from_le_bytes(c_byte))
                .collect::<Vec<u128>>();

            // Perform the consistency check
            for i in 0..sz {
                // let tmp = b[i] - (delta * c[i]); // Rearranged: b[i] == delta * c[i]
                if b[i] != gf128mul(a[i], delta) ^ c[i] {
                    eprintln!("Consistency check failed at index {}", i);
                    panic!("Consistency check failed");
                }
            }
            println!("Consistency check passed");
        }
    }
}

/// Convert delta to a boolean array.
fn delta_to_bool(delta: u128) -> [bool; NUM_BITS] {
    let mut delta_bool = [false; NUM_BITS];
    for i in 0..NUM_BITS {
        delta_bool[i] = (delta >> i) & 1 == 1;
    }
    delta_bool
}
