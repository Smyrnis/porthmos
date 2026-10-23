mod check;
mod digest;
mod fetch;
mod install;
mod manifest;
mod signature;
mod stage;
mod staged;
mod target;
#[cfg(test)]
mod testing;
mod unpack;

pub mod keys;

pub use check::{CheckError, Checker, DEFAULT_MANIFEST_URL, Outcome, UpdateInfo};
pub use digest::{sha256_hex, sha256_hex_of_file};
pub use install::{InstallError, InstallPlace, Installed, forget_previous_version, install, install_place};
pub use manifest::{Artifact, Manifest};
pub use signature::{KeyPair, PublicKey, VerifyError, generate_key_pair, parse_public_key, sign, verify};
pub use stage::{StageError, Stager};
pub use staged::{Staged, UpdateDir, discard_staged, find_staged};
pub use target::running_target;
