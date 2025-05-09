use rand::RngCore;

pub fn rand_u128() -> u128 {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    u128::from_le_bytes(bytes)
}