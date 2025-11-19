use psi_aes::prp::PRP;
use psi_utils::gf128::gf128mul;

pub struct LpnF2k {
    party: usize,
    big_a_idx: Vec<[usize; 10]>,
    big_a_weight: Vec<[u128; 10]>,
}

impl LpnF2k {
    pub fn new(k: usize, n: usize, seed: &[u8; 16], seed_field: &[u8; 16]) -> Self {
        let prp = PRP::new(Some(seed));
        let field_prp = PRP::new(Some(seed_field));
        let mut big_a_idx = vec![[0usize; 10]; n];
        let mut big_a_weight = vec![[0u128; 10]; n];

        big_a_idx.iter_mut().zip(big_a_weight.iter_mut()).enumerate().for_each(|(i, (r, w))| {
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

            let tmp_field: Vec<_> = tmp2
                .iter()
                .map(|x| u128::from_le_bytes(*x))
                .collect();
            w.copy_from_slice(&tmp_field);
        });

        Self {
            party: 0,
            big_a_idx,
            big_a_weight,
        }
    }

    pub fn compute_big_k(&mut self, big_k: &mut [u128], kk_big_k: &[u128]) {
        big_k.iter_mut().enumerate().for_each(|(i, big_ki)| {
            for m in 0..10 {
                *big_ki ^= gf128mul(self.big_a_weight[i][m], kk_big_k[self.big_a_idx[i][m]]);
            }
        });
    }

    pub fn compute_big_k_and_big_m(&mut self, big_k: &mut [u128], big_m: &mut [u128], kk_big_k: &[u128], kk_big_m: &[u128]) {
        big_k.iter_mut().zip(big_m.iter_mut()).enumerate().for_each(|(i, (big_ki, big_mi))| {
            for m in 0..10 {
                *big_ki ^= gf128mul(self.big_a_weight[i][m], kk_big_k[self.big_a_idx[i][m]]);
                *big_mi ^= gf128mul(self.big_a_weight[i][m], kk_big_m[self.big_a_idx[i][m]]);
            }
        });
    }

    pub fn compute_send(&mut self, big_k: &mut [u128], kk_big_k: &[u128]) {
        self.party = 0;
        self.compute_big_k(big_k, kk_big_k);
    }

    pub fn compute_recv(&mut self, big_k: &mut [u128], big_m: &mut [u128], kk_big_k: &[u128], kk_big_m: &[u128]) {
        self.party = 1;
        self.compute_big_k_and_big_m(big_k, big_m, kk_big_k, kk_big_m);
    }
}