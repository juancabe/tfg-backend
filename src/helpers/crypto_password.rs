//! Password hashing and random token generation utilities.

use bcrypt::{DEFAULT_COST, hash, verify};
use openssl::rand::rand_bytes;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hashes a plaintext password using bcrypt with the default cost.
pub fn hash_password(plain: &str) -> Result<String, bcrypt::BcryptError> {
    hash(plain, DEFAULT_COST)
}

/// Verifies a plaintext password against a stored bcrypt hash.
pub fn verify_password(stored_hash: &str, plain: &str) -> bool {
    verify(plain, stored_hash).unwrap_or(false)
}

/// Generates a cryptographically secure random hex string of the given byte length.
pub fn random_token_hex(bytes_len: usize) -> Result<String, openssl::error::ErrorStack> {
    let mut buf = vec![0u8; bytes_len];
    rand_bytes(&mut buf)?;
    Ok(to_hex(&buf))
}
