use openssl::{hash::MessageDigest, memcmp, pkcs5, rand::rand_bytes};

const PBKDF2_ITERATIONS: usize = 120_000;
const SALT_LEN: usize = 16;
const HASH_LEN: usize = 32;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }

    let mut out = Vec::with_capacity(s.len() / 2);
    let chars = s.as_bytes();
    let mut i = 0usize;
    while i < chars.len() {
        let part = std::str::from_utf8(&chars[i..i + 2]).ok()?;
        let byte = u8::from_str_radix(part, 16).ok()?;
        out.push(byte);
        i += 2;
    }
    Some(out)
}

pub fn hash_password(plain: &str) -> Result<String, openssl::error::ErrorStack> {
    let mut salt = [0u8; SALT_LEN];
    rand_bytes(&mut salt)?;

    let mut derived = [0u8; HASH_LEN];
    pkcs5::pbkdf2_hmac(
        plain.as_bytes(),
        &salt,
        PBKDF2_ITERATIONS,
        MessageDigest::sha256(),
        &mut derived,
    )?;

    Ok(format!(
        "pbkdf2_sha256${}${}${}",
        PBKDF2_ITERATIONS,
        to_hex(&salt),
        to_hex(&derived)
    ))
}

pub fn verify_password(stored_hash: &str, plain: &str) -> bool {
    let mut parts = stored_hash.split('$');
    let Some(kind) = parts.next() else {
        return false;
    };
    if kind != "pbkdf2_sha256" {
        return false;
    }

    let Some(iters_s) = parts.next() else {
        return false;
    };
    let Some(salt_hex) = parts.next() else {
        return false;
    };
    let Some(hash_hex) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }

    let Ok(iterations) = iters_s.parse::<usize>() else {
        return false;
    };
    let Some(salt) = from_hex(salt_hex) else {
        return false;
    };
    let Some(expected) = from_hex(hash_hex) else {
        return false;
    };

    let mut actual = vec![0u8; expected.len()];
    if pkcs5::pbkdf2_hmac(
        plain.as_bytes(),
        &salt,
        iterations,
        MessageDigest::sha256(),
        &mut actual,
    )
    .is_err()
    {
        return false;
    }

    memcmp::eq(&actual, &expected)
}

pub fn random_token_hex(bytes_len: usize) -> Result<String, openssl::error::ErrorStack> {
    let mut buf = vec![0u8; bytes_len];
    rand_bytes(&mut buf)?;
    Ok(to_hex(&buf))
}
