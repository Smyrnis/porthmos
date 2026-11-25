use std::time::Duration;

mod arrange;
mod command;
mod model;
mod parse;
mod probe;
mod rate;
#[cfg(test)]
mod testing;

pub use arrange::{Sort, arranged};
pub use command::{
    CommandError, Nice, SHELL_COMMAND, Signal, change_script, may_signal, renice_command, signal_command,
};
pub use model::{Cpu, Disk, Host, Interface, Load, Memory, Process, Section, Snapshot, UNKNOWN_UID};
pub use parse::{Parsed, Sample, parse};
pub use probe::{Token, probe_command};

pub const POLL_INTERVAL: Duration = Duration::from_secs(3);
pub const POLL_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
pub const MAX_PROCESSES: usize = 200;
pub const MAX_LINE_BYTES: usize = 4096;
