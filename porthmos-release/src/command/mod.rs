use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use semver::Version;

use crate::{build_manifest, package};

const SUBCOMMANDS: &str = "keygen, package, manifest, sign, verify";

struct Flags(BTreeMap<String, String>);

impl Flags {
    fn parse(args: &[String]) -> Result<Flags> {
        let mut flags = BTreeMap::new();
        let mut remaining = args.iter();
        while let Some(argument) = remaining.next() {
            let Some(name) = argument.strip_prefix("--") else {
                bail!("expected a --flag, found \"{argument}\"");
            };
            let value = remaining.next().ok_or_else(|| anyhow!("--{name} needs a value"))?;
            flags.insert(name.to_string(), value.clone());
        }
        Ok(Flags(flags))
    }

    fn required(&self, name: &str) -> Result<&str> {
        self.optional(name).ok_or_else(|| anyhow!("missing --{name}"))
    }

    fn optional(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }
}

fn read_file(path: &str) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("couldn't read {path}"))
}

fn write_file(path: &str, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).with_context(|| format!("couldn't write {path}"))
}

fn keygen() -> Result<String> {
    let pair = porthmos_update::generate_key_pair()?;
    Ok(format!("private: {}\npublic: {}", pair.private, pair.public))
}

fn package_command(flags: &Flags) -> Result<String> {
    let target = flags.required("target")?;
    let version: Version = flags.required("version")?.parse().context("--version is not a valid version")?;
    let out = flags.required("out")?;
    package(
        target,
        &version,
        Path::new(flags.required("binary")?),
        Path::new(flags.required("license")?),
        Path::new(out),
    )?;
    let extension = if target.contains("windows") { "zip" } else { "tar.gz" };
    Ok(format!(
        "wrote porthmos-{version}-{target}.{extension}, porthmos-{target}.{extension} and porthmos-{target}.{extension}.sha256 to {out}"
    ))
}

fn manifest_command(flags: &Flags) -> Result<String> {
    let version: Version = flags.required("version")?.parse().context("--version is not a valid version")?;
    let released = DateTime::parse_from_rfc3339(flags.required("released")?)
        .context("--released is not an RFC 3339 time")?
        .with_timezone(&Utc);
    let notes = String::from_utf8(read_file(flags.required("notes-file")?)?).context("the notes are not UTF-8")?;
    let manifest = build_manifest(
        &version,
        released,
        notes.trim(),
        flags.required("base-url")?,
        Path::new(flags.required("artifacts")?),
    )?;
    let out = flags.required("out")?;
    write_file(out, &manifest.to_json()?)?;
    Ok(format!("wrote {out}"))
}

fn sign_command(flags: &Flags, env: &dyn Fn(&str) -> Option<String>) -> Result<String> {
    let variable = flags.required("key-env")?;
    let message = read_file(flags.required("in")?)?;
    let out = flags.required("out")?;
    let key = env(variable).filter(|value| !value.trim().is_empty());
    let key = key.ok_or_else(|| anyhow!("the environment variable {variable} is not set"))?;
    let signature = porthmos_update::sign(&key, &message)
        .map_err(|err| anyhow!("the environment variable {variable} does not hold a usable signing key: {err}"))?;
    write_file(out, format!("{signature}\n").as_bytes())?;
    Ok(format!("wrote {out}"))
}

fn verify_command(flags: &Flags) -> Result<String> {
    let message = read_file(flags.required("in")?)?;
    let signature = String::from_utf8(read_file(flags.required("sig")?)?).context("the signature is not text")?;
    let trusted = match flags.optional("public-key") {
        Some(key) => vec![porthmos_update::parse_public_key(key)?],
        None => porthmos_update::keys::trusted_keys(),
    };
    porthmos_update::verify(&trusted, &message, &signature)?;
    Ok("ok".to_string())
}

pub fn run(args: &[String], env: &dyn Fn(&str) -> Option<String>) -> Result<String> {
    let Some((subcommand, rest)) = args.split_first() else {
        bail!("expected a subcommand: {SUBCOMMANDS}");
    };
    let flags = Flags::parse(rest)?;
    match subcommand.as_str() {
        "keygen" => keygen(),
        "package" => package_command(&flags),
        "manifest" => manifest_command(&flags),
        "sign" => sign_command(&flags, env),
        "verify" => verify_command(&flags),
        other => bail!("unknown subcommand \"{other}\"; expected one of: {SUBCOMMANDS}"),
    }
}

#[cfg(test)]
mod tests;
