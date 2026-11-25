use std::{
    ffi::{OsStr, OsString},
    io::{self, Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    runtime::Handle,
    sync::oneshot,
};

use crate::{TermSize, Transport};

const BRIDGE_BUFFER: usize = 64 * 1024;
const HANGUP_GRACE: Duration = Duration::from_secs(1);
const HANGUP_POLL: Duration = Duration::from_millis(20);
const TERMINAL_TYPE: &str = "xterm-256color";
const COLOR_SUPPORT: &str = "truecolor";

#[derive(Debug, Clone, Default)]
pub struct LocalShell {
    pub program: Option<PathBuf>,
    pub arguments: Vec<OsString>,
    pub directory: Option<PathBuf>,
    pub environment: Vec<(String, String)>,
}

#[cfg(unix)]
pub fn default_shell(lookup: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    lookup("SHELL").filter(|shell| !shell.is_empty()).map_or_else(|| PathBuf::from("/bin/sh"), PathBuf::from)
}

#[cfg(windows)]
pub fn default_shell(lookup: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    lookup("ComSpec").filter(|shell| !shell.is_empty()).map_or_else(|| PathBuf::from("powershell.exe"), PathBuf::from)
}

fn pty_size(size: TermSize) -> PtySize {
    PtySize { rows: size.rows, cols: size.columns, pixel_width: 0, pixel_height: 0 }
}

#[cfg(unix)]
fn hang_up_process_group(leader: Option<u32>) {
    if let Some(leader) = leader.and_then(|pid| i32::try_from(pid).ok()) {
        unsafe {
            libc::killpg(leader, libc::SIGHUP);
        }
    }
}

#[cfg(not(unix))]
fn hang_up_process_group(_: Option<u32>) {}

fn failure(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::other(error)
}

fn command(shell: &LocalShell) -> CommandBuilder {
    let program = shell.program.clone().unwrap_or_else(|| default_shell(|name| std::env::var_os(name)));
    let mut command = CommandBuilder::new(program);
    command.args(&shell.arguments);
    if let Some(directory) = &shell.directory {
        command.cwd(directory);
    }
    command.env("TERM", TERMINAL_TYPE);
    command.env("COLORTERM", COLOR_SUPPORT);
    for (name, value) in &shell.environment {
        command.env(OsStr::new(name), OsStr::new(value));
    }
    command
}

pub fn spawn_local(shell: &LocalShell, size: TermSize) -> io::Result<Transport> {
    let runtime = Handle::try_current().map_err(failure)?;
    let pair = native_pty_system().openpty(pty_size(size)).map_err(|error| failure(error.to_string()))?;
    let mut child = pair.slave.spawn_command(command(shell)).map_err(|error| failure(error.to_string()))?;
    drop(pair.slave);
    let mut pty_reader = pair.master.try_clone_reader().map_err(|error| failure(error.to_string()))?;
    let mut pty_writer = pair.master.take_writer().map_err(|error| failure(error.to_string()))?;
    let mut killer = child.clone_killer();
    let leader = child.process_id();
    let master: Mutex<Box<dyn MasterPty + Send>> = Mutex::new(pair.master);

    let (mut output_sink, output) = tokio::io::duplex(BRIDGE_BUFFER);
    let reading = runtime.clone();
    std::thread::spawn(move || {
        let mut buffer = vec![0u8; BRIDGE_BUFFER];
        while let Ok(count) = pty_reader.read(&mut buffer) {
            if count == 0 || reading.block_on(output_sink.write_all(&buffer[..count])).is_err() {
                break;
            }
        }
    });

    let exited = Arc::new(AtomicBool::new(false));
    let exited_for_writer = exited.clone();
    let (input, mut input_source) = tokio::io::duplex(BRIDGE_BUFFER);
    let writing = runtime;
    std::thread::spawn(move || {
        let mut buffer = vec![0u8; BRIDGE_BUFFER];
        while let Ok(count) = writing.block_on(input_source.read(&mut buffer)) {
            if count == 0 || pty_writer.write_all(&buffer[..count]).is_err() || pty_writer.flush().is_err() {
                break;
            }
        }
        hang_up_process_group(leader);
        let deadline = Instant::now() + HANGUP_GRACE;
        while !exited_for_writer.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(HANGUP_POLL);
        }
        let _ = killer.kill();
    });

    let (report, exit) = oneshot::channel();
    std::thread::spawn(move || {
        let status = child.wait().ok().map(|status| status.exit_code());
        exited.store(true, Ordering::SeqCst);
        let _ = report.send(status);
    });

    let resize = Box::new(move |size: TermSize| {
        if let Ok(master) = master.lock() {
            let _ = master.resize(pty_size(size));
        }
    });
    Ok(Transport { reader: Box::new(output), writer: Box::new(input), resize, exit })
}

#[cfg(test)]
mod tests;
