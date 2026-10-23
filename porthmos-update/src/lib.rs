mod digest;
#[allow(dead_code)]
mod fetch;
mod manifest;
mod signature;
mod target;
#[cfg(test)]
mod testing;

pub mod keys;

pub use digest::{sha256_hex, sha256_hex_of_file};
pub use manifest::{Artifact, Manifest};
pub use signature::{KeyPair, PublicKey, VerifyError, generate_key_pair, parse_public_key, sign, verify};
pub use target::running_target;
