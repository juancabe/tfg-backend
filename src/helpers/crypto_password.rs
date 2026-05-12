use bcrypt::{DEFAULT_COST, hash, verify};
use openssl::rand::rand_bytes;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash_password(plain: &str) -> Result<String, bcrypt::BcryptError> {
    hash(plain, DEFAULT_COST)
}

pub fn verify_password(stored_hash: &str, plain: &str) -> bool {
    verify(plain, stored_hash).unwrap_or(false)
}

pub fn random_token_hex(bytes_len: usize) -> Result<String, openssl::error::ErrorStack> {
    let mut buf = vec![0u8; bytes_len];
    rand_bytes(&mut buf)?;
    Ok(to_hex(&buf))
}
