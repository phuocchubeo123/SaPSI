use psi_aes::ccrh::CCRH;
use psi_network::comm_channel::CommunicationChannel;

// Section 6.1 of https://eprint.iacr.org/2019/074.pdf
// We extend the OT message length by first preparing short OT messages as keys
// And then encrypt the message m^1 || m^2 || ... || m^t into
// H(1 \xor k) \xor m^1 || H(2 \xor k) \xor m^2 || ... || H(t \xor k) \xor m^t


pub struct OTPre<const NUM_LIMBS: usize> {
    pre_data: Vec<[u128; NUM_LIMBS]>,
    bits: Vec<bool>,
    n: usize,
    count: usize,
    length: usize,
    delta: Option<[u8; 16]>,
}

impl<const NUM_LIMBS: usize> OTPre<NUM_LIMBS> {
    /// Create a new `OTPre` instance
    pub fn new(length: usize, times: usize) -> Self {
        let n = length * times;
        Self {
            pre_data: vec![[0u128; NUM_LIMBS]; 2 * n],
            bits: vec![false; n],
            n,
            count: 0,
            length,
            delta: None,
        }
    }

    // Take COT messages already prepared and turn them into random keys for lengthening OT later
    pub fn send_pre(&mut self, data: &[[u8; 16]], delta: [u8; 16]) {
        self.delta = Some(delta);
        let mut pre_hash_data = vec![[0u8; 16]; 2*self.n];
        pre_hash_data[..self.n].copy_from_slice(data);
        for i in self.n..2*self.n {
            pre_hash_data[i] = xor_block(&data[i - self.n], &delta);
        }

        let ccrh = CCRH::new();
        let mut hashed_data = vec![vec![[0u8; 16]; 2*self.n]; NUM_LIMBS];
        for i in 0..NUM_LIMBS {
            let kxori: Vec<[u8; 16]> = pre_hash_data.iter().map(|x| xor_block(x, &[i as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])).collect(); // Get k_i \xor i
            ccrh.Hn(&mut hashed_data[i], &kxori, 2*self.n);
        }
        for i in 0..2*self.n {
            for j in 0..NUM_LIMBS {
                self.pre_data[i][j] = u128::from_le_bytes(hashed_data[j][i]);
            }
        }

        // for i in 0..10 {
        //     println!("This OT:");
        //     println!("{:?}", self.pre_data[i]);
        //     println!("{:?}", self.pre_data[i + self.n]);
        // }
    }

    // Take COT messages already prepared and turn them into random keys for lengthening OT later
    pub fn recv_pre(&mut self, data: &[[u8; 16]], bits: Option<&[bool]>) {
        let ccrh = CCRH::new();
        if let Some(b) = bits {
            self.bits[..self.n].copy_from_slice(b);
        } else {
            for i in 0..self.n {
                self.bits[i] = data[i][0] & 1 != 0; // Extract LSB
            }
        }

        let mut pre_hash_data = vec![[0u8; 16]; self.n];
        pre_hash_data[..self.n].copy_from_slice(data);

        let ccrh = CCRH::new();
        let mut hashed_data = vec![vec![[0u8; 16]; self.n]; NUM_LIMBS];
        for i in 0..NUM_LIMBS {
            let kxori: Vec<[u8; 16]> = pre_hash_data.iter().map(|x| xor_block(x, &[i as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])).collect(); // Get k_i \xor i
            ccrh.Hn(&mut hashed_data[i], &kxori, self.n);
        }
        for i in 0..self.n {
            for j in 0..NUM_LIMBS {
                self.pre_data[i][j] = u128::from_le_bytes(hashed_data[j][i]);
            }
        }

        // for i in 0..10 {
        //     println!("bit: {}", self.bits[i]);
        //     println!("{:?}", self.pre_data[i]);
        // }
    }

    // Spend one more round to send the choice bits
    pub fn choices_sender<IO: CommunicationChannel>(&mut self, io: &mut IO, comm: &mut u64) {
        let received_bits = io.receive_bits().expect("Failed to receive bits");
        for (i, &bit) in received_bits.iter().enumerate() {
            self.bits[self.count + i] = bit;
        }
        self.count += self.length;
    }

    pub fn choices_recver<IO: CommunicationChannel>(&mut self, io: &mut IO, choices: &[bool], comm: &mut u64) {
        let mut adjusted_bits = vec![false; self.length];
        for i in 0..self.length {
            adjusted_bits[i] = choices[i] ^ self.bits[self.count + i];
            self.bits[self.count+i] = adjusted_bits[i].clone();
        }
        *comm += io.send_bits(&adjusted_bits).expect("Failed to send bits");
        self.count += self.length;
    }


    /// Send data based on precomputed values
    /// TODO: Change to arbitrary message length
    pub fn send<IO: CommunicationChannel>(
        &mut self,
        io: &mut IO,
        m0: &[[u128; NUM_LIMBS]],
        m1: &[[u128; NUM_LIMBS]],
        length: usize,
        s: usize,
        comm: &mut u64,
    ) {
        let mut pad = vec![[0u128; NUM_LIMBS]; 2*length];
        let k = s * length;

        for i in 0..length {
            let idx = k + i;
            if !self.bits[idx] {
                pad[2*i] = xor_message::<NUM_LIMBS>(&m0[i], &self.pre_data[idx]);
                pad[2*i+1] = xor_message::<NUM_LIMBS>(&m1[i], &self.pre_data[idx + self.n]);
            } else {
                pad[2*i] = xor_message::<NUM_LIMBS>(&m0[i], &self.pre_data[idx + self.n]);
                pad[2*i+1] = xor_message::<NUM_LIMBS>(&m1[i], &self.pre_data[idx]);
            }
        }
        *comm += io.send_16byte_block::<NUM_LIMBS>(&pad).expect("Failed to send padded data");
    }

    /// Receive and reconstruct data based on precomputed values
    pub fn recv<IO: CommunicationChannel>(
        &mut self,
        io: &mut IO,
        data: &mut [[u128; NUM_LIMBS]],
        b: &[bool],
        length: usize,
        s: usize,
        comm: &mut u64,
    ) {
        let pad = io.receive_16byte_block::<NUM_LIMBS>().expect("Receive padded data failed");
        let k = s * length;

        for i in 0..length {
            let idx = if b[i] { 1 } else { 0 };
            data[i] = xor_message::<NUM_LIMBS>(&self.pre_data[k + i], &pad[2*i + idx]);
        }
    }

    /// Reset the internal counter
    pub fn reset(&mut self) {
        self.count = 0;
    }
}

fn xor_block(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    [
        a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3],
        a[4] ^ b[4], a[5] ^ b[5], a[6] ^ b[6], a[7] ^ b[7],
        a[8] ^ b[8], a[9] ^ b[9], a[10] ^ b[10], a[11] ^ b[11],
        a[12] ^ b[12], a[13] ^ b[13], a[14] ^ b[14], a[15] ^ b[15]
    ]
}

fn xor_message<const NUM_LIMBS: usize>(a: &[u128; NUM_LIMBS], b: &[u128; NUM_LIMBS]) -> [u128; NUM_LIMBS] {
    let mut res = [0u128; NUM_LIMBS];
    for i in 0..NUM_LIMBS {
        res[i] = a[i] ^ b[i];
    }
    res
}