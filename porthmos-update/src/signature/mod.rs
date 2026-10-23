use std::fmt;

use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

const KEY_LENGTH: usize = 32;
const SIGNATURE_LENGTH: usize = 64;

pub struct KeyPair {
    pub private: String,
    pub public: String,
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair").field("private", &"<redacted>").field("public", &self.public).finish()
    }
}

#[derive(Debug, Clone)]
pub struct PublicKey(VerifyingKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    NoTrustedKeys,
    BadEncoding,
    Mismatch,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            VerifyError::NoTrustedKeys => "no trusted signing keys are available",
            VerifyError::BadEncoding => "the signature is not valid base64 of 64 bytes",
            VerifyError::Mismatch => "the signature does not match",
        })
    }
}

impl std::error::Error for VerifyError {}

fn decode_key(text: &str, what: &str) -> Result<[u8; KEY_LENGTH]> {
    let bytes = STANDARD.decode(text.trim()).map_err(|_| anyhow!("the {what} is not valid base64"))?;
    bytes.try_into().map_err(|_| anyhow!("the {what} must be {KEY_LENGTH} bytes"))
}

pub fn generate_key_pair() -> Result<KeyPair> {
    let mut seed = [0u8; KEY_LENGTH];
    getrandom::fill(&mut seed).map_err(|err| anyhow!("no random numbers available: {err}"))?;
    let key = SigningKey::from_bytes(&seed);
    Ok(KeyPair { private: STANDARD.encode(seed), public: STANDARD.encode(key.verifying_key().to_bytes()) })
}

pub fn sign(private_key: &str, message: &[u8]) -> Result<String> {
    let key = SigningKey::from_bytes(&decode_key(private_key, "private key")?);
    Ok(STANDARD.encode(key.sign(message).to_bytes()))
}

pub fn parse_public_key(base64: &str) -> Result<PublicKey> {
    let bytes = decode_key(base64, "public key")?;
    VerifyingKey::from_bytes(&bytes).map(PublicKey).map_err(|_| anyhow!("the public key is not a valid Ed25519 key"))
}

pub fn verify(trusted: &[PublicKey], message: &[u8], signature: &str) -> Result<(), VerifyError> {
    if trusted.is_empty() {
        return Err(VerifyError::NoTrustedKeys);
    }
    let bytes = STANDARD.decode(signature.trim()).map_err(|_| VerifyError::BadEncoding)?;
    let bytes: [u8; SIGNATURE_LENGTH] = bytes.try_into().map_err(|_| VerifyError::BadEncoding)?;
    let signature = Signature::from_bytes(&bytes);
    if trusted.iter().any(|key| key.0.verify_strict(message, &signature).is_ok()) {
        Ok(())
    } else {
        Err(VerifyError::Mismatch)
    }
}

#[cfg(test)]
mod tests;
