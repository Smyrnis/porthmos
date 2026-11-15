use std::time::Duration;

pub mod digest;
pub mod measure;
pub mod text;

pub use digest::Digest;
pub use measure::Measured;

pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
