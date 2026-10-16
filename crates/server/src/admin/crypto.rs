//! TOTP secrets use authenticated encryption bound to their account; no secret appears in logs.
use super::Config;
use crate::online::{Error, Result};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use uuid::Uuid;

fn associated(user: Uuid) -> Vec<u8> {
    let mut data = b"spellgambit-admin-totp-v1:".to_vec();
    data.extend_from_slice(user.as_bytes());
    data
}
fn cipher(config: &Config) -> Result<Aes256Gcm> {
    let key = crate::auth::crypto::sign(&config.key, b"spellgambit-admin-totp-cipher-key-v1");
    Aes256Gcm::new_from_slice(&key).map_err(|_| Error("INTERNAL"))
}
pub(super) fn seal(config: &Config, user: Uuid, secret: &[u8]) -> Result<Vec<u8>> {
    if secret.len() != 20 {
        return Err(Error("INVALID_MESSAGE"));
    }
    let cipher = cipher(config)?;
    let mut nonce = [0; 12];
    getrandom::fill(&mut nonce).map_err(|_| Error("INTERNAL"))?;
    let encrypted = cipher
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: secret,
                aad: &associated(user),
            },
        )
        .map_err(|_| Error("INTERNAL"))?;
    let mut result = nonce.to_vec();
    result.extend(encrypted);
    Ok(result)
}
pub(super) fn open(config: &Config, user: Uuid, sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() != 48 {
        return Err(Error("INTERNAL"));
    }
    let cipher = cipher(config)?;
    let nonce: [u8; 12] = sealed[..12].try_into().map_err(|_| Error("INTERNAL"))?;
    cipher
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: &sealed[12..],
                aad: &associated(user),
            },
        )
        .map_err(|_| Error("INTERNAL"))
}
pub(super) fn totp(secret: Vec<u8>, login: &str) -> Result<totp_rs::Totp> {
    totp_rs::Builder::new()
        .with_algorithm(totp_rs::Algorithm::SHA1)
        .with_digits(6)
        .with_step_duration(30)
        .with_skew(1)
        .with_secret(secret)
        .with_issuer(Some("Spellgambit"))
        .with_account_name(login)
        .build()
        .map_err(|_| Error("INTERNAL"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ciphertext_is_random_bound_to_the_owner_and_rejects_tampering() {
        let config = Config::new([7; 32], vec!["127.0.0.1/32".parse().unwrap()]).unwrap();
        let user = Uuid::now_v7();
        let secret = [9; 20];
        let sealed = seal(&config, user, &secret).unwrap();
        assert_eq!(sealed.len(), 48);
        assert_ne!(sealed, seal(&config, user, &secret).unwrap());
        assert_eq!(open(&config, user, &sealed).unwrap(), secret);
        assert!(open(&config, Uuid::now_v7(), &sealed).is_err());
        let mut corrupt = sealed;
        corrupt[25] ^= 1;
        assert!(open(&config, user, &corrupt).is_err());
    }
}
