use std::fmt;

use crate::model::UNKNOWN_UID;

const MIN_NICE: i32 = -20;
const MAX_NICE: i32 = 19;
const FIRST_SIGNALLABLE_PID: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Terminate,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nice(i32);

impl Nice {
    pub fn new(value: i32) -> Option<Self> {
        (MIN_NICE..=MAX_NICE).contains(&value).then_some(Self(value))
    }

    pub fn get(self) -> i32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    PidNotAllowed,
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PidNotAllowed => formatter.write_str("that process may not be changed"),
        }
    }
}

impl std::error::Error for CommandError {}

fn checked(pid: u32) -> Result<u32, CommandError> {
    (pid >= FIRST_SIGNALLABLE_PID).then_some(pid).ok_or(CommandError::PidNotAllowed)
}

pub fn signal_command(pid: u32, signal: Signal) -> Result<String, CommandError> {
    let name = match signal {
        Signal::Terminate => "TERM",
        Signal::Kill => "KILL",
    };
    Ok(format!("kill -{name} {}", checked(pid)?))
}

pub fn renice_command(pid: u32, nice: Nice) -> Result<String, CommandError> {
    Ok(format!("renice {} -p {}", nice.get(), checked(pid)?))
}

pub const SHELL_COMMAND: &str = "sh -s";

pub fn change_script(command: &str) -> String {
    format!("LC_ALL=C; export LC_ALL\n{command}\n")
}

pub fn may_signal(connection_uid: u32, process_uid: u32) -> bool {
    connection_uid == 0 || (connection_uid != UNKNOWN_UID && connection_uid == process_uid)
}

#[cfg(test)]
mod tests;
