use std::{
    fmt::Write as _,
    fs::File,
    io::{self, Read},
    path::Path,
};

use sha2::{Digest, Sha256};

const CHUNK: usize = 64 * 1024;

pub(crate) fn hex(digest: &[u8]) -> String {
    digest.iter().fold(String::with_capacity(digest.len() * 2), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub fn sha256_hex_of_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

#[cfg(test)]
mod tests;
