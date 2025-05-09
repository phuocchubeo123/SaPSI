use psi_aes::prp::PRP;
use psi_utils::gf128::gf128mul;

pub struct LpnF2k {
    party: usize,
    k: usize, 
    n: usize,
    M: Vec<u128>,
    preM: Vec<u128>,
    K: Vec<u128>,
    preK: Vec<u128>,
    A_idx: Vec<[usize; 10]>,
    A_weight: Vec<[u128; 10]>,
}

impl LpnF2k {
    pub fn new(k: usize, n: usize, seed: &[u8; 16], seed_field: &[u8; 16]) -> Self {
        let prp = PRP::new(Some(seed));
        let field_prp = PRP::new(Some(seed_field));
        let mut A_idx = vec![[0usize; 10]; n];
        let mut A_weight = vec![[0u128; 10]; n];

        A_idx.iter_mut().zip(A_weight.iter_mut()).enumerate().for_each(|(i, (r, w))| {
            let mut tmp = vec![[0u8; 16]; 10];
            let mut tmp2 = vec![[0u8; 16]; 10];
            for m in 0..10 {
                tmp[m][0..8].copy_from_slice(&(i).to_le_bytes());
                tmp[m][8..].copy_from_slice(&(m as usize).to_le_bytes());
                tmp2[m][0..8].copy_from_slice(&(i).to_le_bytes());
                tmp2[m][8..16].copy_from_slice(&(m as usize).to_le_bytes());
            }

            prp.permute_block(&mut tmp, 10);
            let r1: Vec<usize> = tmp
                .iter()
                .map(|x| ((u128::from_le_bytes(*x) >> 64) as usize) % k)
                .collect();

            r.copy_from_slice(&r1);

            field_prp.permute_block(&mut tmp2, 10);

            let mut tmp_field: Vec<_> = tmp2
                .iter()
                .map(|x| u128::from_le_bytes(*x))
                .collect();
            w.copy_from_slice(&tmp_field);
        });

        Self {
            party: 0,
            k: k,
            n: n,
            M: vec![0u128; n],
            preM: vec![0u128; k],
            K: vec![0u128; n],
            preK: vec![0u128; k],
            A_idx: A_idx,
            A_weight: A_weight,
        }
    }

    pub fn compute_K(&mut self, K: &mut [u128], kkK: &[u128]) {
        K.iter_mut().enumerate().for_each(|(i, Ki)| {
            for m in 0..10 {
                *Ki ^= gf128mul(self.A_weight[i][m], kkK[self.A_idx[i][m]]);
            }
        });
    }

    pub fn compute_K_and_M(&mut self, K: &mut [u128], M: &mut [u128], kkK: &[u128], kkM: &[u128]) {
        K.iter_mut().zip(M.iter_mut()).enumerate().for_each(|(i, (Ki, Mi))| {
            for m in 0..10 {
                *Ki ^= gf128mul(self.A_weight[i][m], kkK[self.A_idx[i][m]]);
                *Mi ^= gf128mul(self.A_weight[i][m], kkM[self.A_idx[i][m]]);
            }
        });
    }

    pub fn compute_send(&mut self, K: &mut [u128], kkK: &[u128]) {
        self.party = 0;
        self.compute_K(K, kkK);
    }

    pub fn compute_recv(&mut self, K: &mut [u128], M: &mut [u128], kkK: &[u128], kkM: &[u128]) {
        self.party = 1;
        self.compute_K_and_M(K, M, kkK, kkM);
    }
}