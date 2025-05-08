use aes::{Aes128, Aes256};
use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};
use lambdaworks_math::field::fields::fft_friendly::stark_252_prime_field::Stark252PrimeField;
use lambdaworks_math::field::element::FieldElement;
use lambdaworks_math::traits::ByteConversion;

pub type F = Stark252PrimeField;
pub type FE = FieldElement<F>;

pub struct TwoKeyPRP {
}

impl TwoKeyPRP {
    pub fn new() -> Self {
        TwoKeyPRP {}
    }

    /// Expand a single parent field element into two child field elements.
    pub fn node_expand_1to2(&self, children: &mut [FE], parent: &FE) {
        assert_eq!(children.len(), 2, "Node expand from 1 to 2 expects children to be an array of size 2.");

        let parent_bytes = parent.to_bytes_le();
        let aes_key = Aes256::new(GenericArray::from_slice(&parent_bytes));

        // Prepare a buffer for 4 blocks (16 bytes each)
        let mut expanded_parent: [_; 4] = core::array::from_fn(|i| GenericArray::clone_from_slice(&[i as u8; 16]));

        // Encrypt the 4 blocks using the AES key
        aes_key.encrypt_blocks(&mut expanded_parent);

        // Combine the first 2 blocks into `left_child` (32 bytes)
        let left_child_bytes = [expanded_parent[0].as_slice(), expanded_parent[1].as_slice()].concat();

        // Combine the next 2 blocks into `right_child` (32 bytes)
        let right_child_bytes = [expanded_parent[2].as_slice(), expanded_parent[3].as_slice()].concat();

        // Convert the byte arrays back into field elements
        children[0] = FE::from_bytes_le(&left_child_bytes).expect("Failed to convert left_child bytes to field element");
        children[1] = FE::from_bytes_le(&right_child_bytes).expect("Failed to convert right_child bytes to field element");
    }

    /// Unrolled version: Expand two parent field elements into four child field elements.
    pub fn node_expand_2to4(&self, children: &mut [FE], parents: &[FE]) {
        let mut temp_children = [FE::zero(); 2];

        // Expand first parent into the first two children
        self.node_expand_1to2(&mut temp_children, &parents[0]);
        children[0] = temp_children[0].clone();
        children[1] = temp_children[1].clone();

        // Expand second parent into the next two children
        self.node_expand_1to2(&mut temp_children, &parents[1]);
        children[2] = temp_children[0].clone();
        children[3] = temp_children[1].clone();
    }

    /// Unrolled version: Expand four parent field elements into eight child field elements.
    pub fn node_expand_4to8(&self, children: &mut [FE; 8], parents: &[FE; 4]) {
        let mut temp_children = [FE::zero(); 2];

        // Expand first parent into the first two children
        self.node_expand_1to2(&mut temp_children, &parents[0]);
        children[0] = temp_children[0].clone();
        children[1] = temp_children[1].clone();

        // Expand second parent into the next two children
        self.node_expand_1to2(&mut temp_children, &parents[1]);
        children[2] = temp_children[0].clone();
        children[3] = temp_children[1].clone();

        // Expand third parent into the next two children
        self.node_expand_1to2(&mut temp_children, &parents[2]);
        children[4] = temp_children[0].clone();
        children[5] = temp_children[1].clone();

        // Expand fourth parent into the last two children
        self.node_expand_1to2(&mut temp_children, &parents[3]);
        children[6] = temp_children[0].clone();
        children[7] = temp_children[1].clone();
    }
}

pub struct TwoKeyPRPF2k {
    aes_key: [Aes128; 2],
}

impl TwoKeyPRPF2k {
    pub fn new(keys: [[u8; 16]; 2]) -> Self {
        let aes_key_0 = Aes128::new(GenericArray::from_slice(&keys[0]));
        let aes_key_1 = Aes128::new(GenericArray::from_slice(&keys[1]));

        TwoKeyPRPF2k {
            aes_key: [aes_key_0, aes_key_1],
        }
    }

    pub fn expand_left(&self, old: &[u128], new: &mut [u128]) {
        let mut old_bytes: Vec<[u8; 16]> = old.iter().map(|x| x.to_le_bytes()).collect();
        let mut old_array: Vec<_> = old_bytes.iter().map(|x| GenericArray::clone_from_slice(x)).collect();
        self.aes_key[0].encrypt_blocks(&mut old_array);
        new.iter_mut().enumerate().for_each(|(i, x)| {
            let mut new_x = [0u8; 16];
            new_x.copy_from_slice(old_array[i].as_slice());
            *x = u128::from_le_bytes(new_x);
        });
    }

    pub fn expand_right(&self, old: &[u128], new: &mut [u128]) {
        let mut old_bytes: Vec<[u8; 16]> = old.iter().map(|x| x.to_le_bytes()).collect();
        let mut old_array: Vec<_> = old_bytes.iter().map(|x| GenericArray::clone_from_slice(x)).collect();
        self.aes_key[1].encrypt_blocks(&mut old_array);
        new.iter_mut().enumerate().for_each(|(i, x)| {
            let mut new_x = [0u8; 16];
            new_x.copy_from_slice(old_array[i].as_slice());
            *x = u128::from_le_bytes(new_x);
        });
    }
}