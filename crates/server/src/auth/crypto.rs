use std::sync::LazyLock;

use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};
use argon2::{Algorithm, Argon2, Params, Version};
use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::{Digest as _, Sha256};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn unhex(value: &str) -> Option<Vec<u8>> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..64)
        .step_by(2)
        .map(|offset| u8::from_str_radix(&value[offset..offset + 2], 16).ok())
        .collect()
}

pub fn token() -> anyhow::Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| anyhow::anyhow!("OS random generator unavailable"))?;
    Ok(hex(&bytes))
}

pub fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

pub fn sign(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub fn verify(key: &[u8], data: &[u8], signature: &[u8]) -> bool {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.verify_slice(signature).is_ok()
}

fn argon() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19456, 2, 1, None).expect("fixed parameters"),
    )
}

pub fn password_hash(password: &str) -> anyhow::Result<String> {
    argon()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| anyhow::anyhow!("password hashing failed"))
}

pub fn password_matches(password: &str, stored: Option<&str>) -> bool {
    static DUMMY: LazyLock<String> = LazyLock::new(|| {
        password_hash("unused random dummy credential").expect("OS random generator")
    });
    let encoded = stored.unwrap_or(&DUMMY);
    let valid = PasswordHash::new(encoded)
        .is_ok_and(|hash| argon().verify_password(password.as_bytes(), &hash).is_ok());
    valid && stored.is_some()
}

pub fn password_allowed(password: &str) -> bool {
    (10..=128).contains(&password.chars().count())
        && password.len() <= 512
        && !include_str!("common-passwords.txt")
            .lines()
            .any(|blocked| blocked.eq_ignore_ascii_case(password))
}
