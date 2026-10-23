use std::{
    fmt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use anyhow::Result;
use reqwest::{Url, redirect::Policy};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FetchError {
    NotHttps,
    Status(u16),
    TooLarge { limit: usize },
    SizeMismatch { expected: u64, actual: u64 },
    HashMismatch,
    Cancelled,
    Disk(String),
    Network(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::NotHttps => f.write_str("the address does not start with https://"),
            FetchError::Status(code) => write!(f, "the server answered {code}"),
            FetchError::TooLarge { limit } => write!(f, "the answer is larger than {limit} bytes"),
            FetchError::SizeMismatch { expected, actual } => {
                write!(f, "the download is {actual} bytes but {expected} were expected")
            }
            FetchError::HashMismatch => f.write_str("the download does not match its checksum"),
            FetchError::Cancelled => f.write_str("the download was cancelled"),
            FetchError::Disk(text) => write!(f, "couldn't write the download: {text}"),
            FetchError::Network(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for FetchError {}

pub(crate) struct Fetcher {
    client: reqwest::Client,
    https_only: bool,
}

fn redirect_allowed(https_only: bool, next: &Url, hops: usize) -> bool {
    hops < MAX_REDIRECTS && (!https_only || next.scheme() == "https")
}

fn classify(error: &reqwest::Error) -> FetchError {
    let text = if error.is_timeout() {
        "the request timed out"
    } else if error.is_connect() {
        "couldn't connect to the server"
    } else if error.is_redirect() {
        "the server redirected too often or somewhere not allowed"
    } else {
        "the request failed"
    };
    FetchError::Network(text.to_string())
}

fn starts_with_https(url: &str) -> bool {
    url.get(..8).is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
}

impl Fetcher {
    pub(crate) fn new(user_agent: &str, timeout: Duration, https_only: bool) -> Result<Fetcher> {
        let policy = Policy::custom(move |attempt| {
            if redirect_allowed(https_only, attempt.url(), attempt.previous().len()) {
                attempt.follow()
            } else {
                attempt.error("redirect refused")
            }
        });
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(timeout)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(policy)
            .referer(false)
            .build()?;
        Ok(Fetcher { client, https_only })
    }

    pub(crate) async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, FetchError> {
        if self.https_only && !starts_with_https(url) {
            return Err(FetchError::NotHttps);
        }
        let mut response = self.client.get(url).send().await.map_err(|error| classify(&error))?;
        if !response.status().is_success() {
            return Err(FetchError::Status(response.status().as_u16()));
        }
        if response.content_length().is_some_and(|length| length > limit as u64) {
            return Err(FetchError::TooLarge { limit });
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| classify(&error))? {
            if body.len() + chunk.len() > limit {
                return Err(FetchError::TooLarge { limit });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}

fn disk_failure(error: std::io::Error) -> FetchError {
    FetchError::Disk(error.kind().to_string())
}

impl Fetcher {
    pub(crate) async fn get_to_file(
        &self, url: &str, path: &Path, size: u64, sha256: &str, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64),
    ) -> Result<(), FetchError> {
        if self.https_only && !starts_with_https(url) {
            return Err(FetchError::NotHttps);
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(FetchError::Cancelled);
        }
        let response = self.client.get(url).send().await.map_err(|error| classify(&error))?;
        if !response.status().is_success() {
            return Err(FetchError::Status(response.status().as_u16()));
        }
        if let Some(declared) = response.content_length().filter(|declared| *declared != size) {
            return Err(FetchError::SizeMismatch { expected: size, actual: declared });
        }
        let mut file = tokio::fs::File::create(path).await.map_err(disk_failure)?;
        let outcome = write_body(response, &mut file, size, sha256, cancel, progress).await;
        drop(file);
        if outcome.is_err() {
            let _ = tokio::fs::remove_file(path).await;
        }
        outcome
    }
}

async fn write_body(
    mut response: reqwest::Response, file: &mut tokio::fs::File, size: u64, sha256: &str, cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), FetchError> {
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(FetchError::Cancelled);
        }
        let Some(chunk) = response.chunk().await.map_err(|error| classify(&error))? else { break };
        total += chunk.len() as u64;
        if total > size {
            return Err(FetchError::SizeMismatch { expected: size, actual: total });
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(disk_failure)?;
        progress(total, size);
    }
    if total != size {
        return Err(FetchError::SizeMismatch { expected: size, actual: total });
    }
    file.flush().await.map_err(disk_failure)?;
    if crate::digest::hex(&hasher.finalize()) != sha256 {
        return Err(FetchError::HashMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
