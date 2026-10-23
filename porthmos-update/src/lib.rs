mod manifest;
mod signature;

pub mod keys;

pub use manifest::{Artifact, Manifest};
pub use signature::{KeyPair, PublicKey, VerifyError, generate_key_pair, parse_public_key, sign, verify};
