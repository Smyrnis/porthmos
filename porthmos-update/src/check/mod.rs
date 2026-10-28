use std::{fmt, time::Duration};

use chrono::{DateTime, Utc};
use semver::Version;

use crate::{
    Artifact, Manifest, PublicKey, VerifyError,
    fetch::{FetchError, Fetcher},
    keys, running_target, verify,
};

const MANIFEST_LIMIT: usize = 262_144;
const SIGNATURE_LIMIT: usize = 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub const DEFAULT_MANIFEST_URL: &str = "https://github.com/Smyrnis/porthmos/releases/latest/download/latest.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: Version,
    pub released: DateTime<Utc>,
    pub notes: String,
    pub artifact: Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    UpToDate,
    Available(UpdateInfo),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    Fetch(String),
    Signature(VerifyError),
    Manifest(String),
    NoBuildForTarget { version: Version, target: String },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::Fetch(text) => write!(f, "couldn't check for updates: {text}"),
            CheckError::Signature(error) => write!(f, "the update information is not signed by a trusted key: {error}"),
            CheckError::Manifest(text) => write!(f, "the update information is not valid: {text}"),
            CheckError::NoBuildForTarget { version, target } => {
                write!(f, "version {version} has no build for {target}")
            }
        }
    }
}

impl std::error::Error for CheckError {}

pub struct Checker {
    fetcher: Fetcher,
    manifest_url: String,
    trusted: Vec<PublicKey>,
    target: String,
}

fn fetch_failed(error: FetchError) -> CheckError {
    CheckError::Fetch(error.to_string())
}

impl Checker {
    pub fn new(manifest_url: &str, trusted: Vec<PublicKey>, target: &str, user_agent: &str) -> anyhow::Result<Checker> {
        Checker::with_policy(manifest_url, trusted, target, user_agent, true)
    }

    fn with_policy(
        manifest_url: &str, trusted: Vec<PublicKey>, target: &str, user_agent: &str, https_only: bool,
    ) -> anyhow::Result<Checker> {
        Ok(Checker {
            fetcher: Fetcher::new(user_agent, REQUEST_TIMEOUT, https_only)?,
            manifest_url: manifest_url.to_string(),
            trusted,
            target: target.to_string(),
        })
    }

    pub fn release(user_agent: &str) -> anyhow::Result<Checker> {
        Checker::new(DEFAULT_MANIFEST_URL, keys::trusted_keys(), running_target(), user_agent)
    }

    pub async fn check(&self, current: &Version) -> Result<Outcome, CheckError> {
        let manifest_bytes = self.fetcher.get(&self.manifest_url, MANIFEST_LIMIT).await.map_err(fetch_failed)?;
        let signature_url = format!("{}.sig", self.manifest_url);
        let signature_bytes = self.fetcher.get(&signature_url, SIGNATURE_LIMIT).await.map_err(fetch_failed)?;
        let signature =
            String::from_utf8(signature_bytes).map_err(|_| CheckError::Signature(VerifyError::BadEncoding))?;
        verify(&self.trusted, &manifest_bytes, &signature).map_err(CheckError::Signature)?;

        let manifest = Manifest::parse(&manifest_bytes).map_err(|error| CheckError::Manifest(error.to_string()))?;
        if !manifest.supersedes(current) {
            return Ok(Outcome::UpToDate);
        }
        let artifact = manifest.artifact_for(&self.target).cloned().ok_or_else(|| CheckError::NoBuildForTarget {
            version: manifest.version.clone(),
            target: self.target.clone(),
        })?;
        Ok(Outcome::Available(UpdateInfo {
            version: manifest.version,
            released: manifest.released,
            notes: manifest.notes,
            artifact,
        }))
    }

    #[cfg(test)]
    fn trusted_count(&self) -> usize {
        self.trusted.len()
    }
}

#[cfg(test)]
mod tests;
