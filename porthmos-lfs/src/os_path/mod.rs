#[cfg(not(windows))]
mod unix;
mod validation;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use unix::{from_os_path, to_os_path};
#[cfg(windows)]
pub use windows::{from_os_path, to_os_path};

pub(crate) use validation::checked_os_path;

#[cfg(test)]
mod tests;
