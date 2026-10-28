use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use porthmos_update::{Artifact, Manifest, sha256_hex_of_file};
use semver::Version;

const ARCHIVE_SUFFIXES: [&str; 2] = [".tar.gz", ".zip"];

fn target_of<'a>(file_name: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = file_name.strip_prefix(prefix)?;
    ARCHIVE_SUFFIXES.iter().find_map(|suffix| rest.strip_suffix(suffix)).filter(|target| !target.is_empty())
}

pub fn build_manifest(
    version: &Version, released: DateTime<Utc>, notes: &str, base_url: &str, artifacts: &Path,
) -> Result<Manifest> {
    let prefix = format!("porthmos-{version}-");
    let base = base_url.trim_end_matches('/');
    let mut targets = BTreeMap::new();

    for entry in fs::read_dir(artifacts).with_context(|| format!("couldn't read {}", artifacts.display()))? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some((file_name, target)) =
            file_name.to_str().and_then(|name| target_of(name, &prefix).map(|target| (name, target)))
        else {
            continue;
        };
        let artifact = Artifact {
            url: format!("{base}/{file_name}"),
            sha256: sha256_hex_of_file(&entry.path()).with_context(|| format!("couldn't read {file_name}"))?,
            size: entry.metadata()?.len(),
        };
        if targets.insert(target.to_string(), artifact).is_some() {
            bail!("more than one archive for target \"{target}\"");
        }
    }
    if targets.is_empty() {
        bail!("no archives named {prefix}<target>.tar.gz or .zip found in {}", artifacts.display());
    }

    let manifest = Manifest { version: version.clone(), released, notes: notes.to_string(), targets };
    Manifest::parse(&manifest.to_json()?)
}

#[cfg(test)]
mod tests;
