use std::collections::BTreeMap;

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: Version,
    pub released: DateTime<Utc>,
    pub notes: String,
    pub targets: BTreeMap<String, Artifact>,
}

fn is_lowercase_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl Artifact {
    fn validate(&self, target: &str) -> Result<()> {
        if !self.url.starts_with("https://") {
            bail!("target \"{target}\": url must start with https://");
        }
        if !is_lowercase_sha256(&self.sha256) {
            bail!("target \"{target}\": sha256 must be 64 lowercase hex characters");
        }
        if self.size == 0 {
            bail!("target \"{target}\": size must be greater than zero");
        }
        Ok(())
    }
}

impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Manifest> {
        let manifest: Manifest = serde_json::from_slice(bytes)?;
        if manifest.targets.is_empty() {
            bail!("the manifest lists no targets");
        }
        for (target, artifact) in &manifest.targets {
            artifact.validate(target)?;
        }
        Ok(manifest)
    }

    pub fn artifact_for(&self, target: &str) -> Option<&Artifact> {
        self.targets.get(target)
    }

    pub fn supersedes(&self, current: &Version) -> bool {
        self.version > *current && (self.version.pre.is_empty() || !current.pre.is_empty())
    }

    pub fn to_json(&self) -> Result<Vec<u8>> {
        let mut json = serde_json::to_vec_pretty(self)?;
        json.push(b'\n');
        Ok(json)
    }
}

#[cfg(test)]
mod tests;
