use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};

use anyhow::{Context, Result};
use tracing::{Level, Metadata, Subscriber};
use tracing_subscriber::{
    EnvFilter,
    filter::{FilterExt, Targets, filter_fn},
    layer::{Filter, Layer, SubscriberExt},
    registry::LookupSpan,
};

const TRANSFER_TARGET: &str = "porthmos::transfers";

pub fn log_filter<S>(directives: Option<&str>) -> Box<dyn Filter<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    match directives {
        Some(directives) if !directives.trim().is_empty() => with_transfer_log(EnvFilter::new(directives)).boxed(),
        _ => EnvFilter::new("error").or(filter_fn(is_porthmos_warning)).or(transfer_errors()).boxed(),
    }
}

fn is_porthmos_warning(metadata: &Metadata<'_>) -> bool {
    metadata.target().starts_with("porthmos") && *metadata.level() <= Level::WARN
}

fn transfer_errors() -> Targets {
    Targets::new().with_target(TRANSFER_TARGET, Level::ERROR)
}

pub fn with_transfer_log<S>(filter: EnvFilter) -> impl Filter<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    filter.or(transfer_errors())
}

pub fn file_subscriber(file: File, directives: Option<&str>) -> impl Subscriber + Send + Sync {
    tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .with_filter(log_filter(directives)),
    )
}

pub fn init(program: &str, log_file: Option<&Path>) {
    let directives = std::env::var("RUST_LOG").ok();
    let writer = match log_file {
        Some(path) => open_writer_at(path),
        None => Err(anyhow::anyhow!("HOME environment variable is not set")),
    };
    let installed = match writer {
        Ok(file) => tracing::subscriber::set_global_default(file_subscriber(file, directives.as_deref())),
        Err(err) => {
            eprintln!("{program}: failed to open log file, logging disabled: {err}");
            tracing::subscriber::set_global_default(
                tracing_subscriber::registry().with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(std::io::sink)
                        .with_filter(log_filter(directives.as_deref())),
                ),
            )
        }
    };
    if installed.is_err() {
        eprintln!("{program}: a logger was already installed");
    }
}

pub fn open_writer_at(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open log file at {}", path.display()))
}

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests;
