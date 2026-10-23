use crate::signature::{PublicKey, parse_public_key};

pub const TRUSTED_PUBLIC_KEYS: &[&str] = &[];

pub fn trusted_keys() -> Vec<PublicKey> {
    TRUSTED_PUBLIC_KEYS.iter().map(|key| parse_public_key(key).expect("an embedded public key is invalid")).collect()
}

#[cfg(test)]
mod tests;
