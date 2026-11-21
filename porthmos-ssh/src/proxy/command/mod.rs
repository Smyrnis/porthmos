use std::{
    future::Future,
    io,
    pin::Pin,
    process::Stdio,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, Join, ReadBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    sync::watch,
    time::Sleep,
};

const FIRST_BYTE_TIME: Duration = Duration::from_secs(30);
const MAX_STDERR_BYTES: usize = 2048;
const MAX_DIAGNOSTIC_CHARS: usize = 300;
const DIAGNOSTIC_WAIT: Duration = Duration::from_millis(300);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) fn safe_in_a_command(value: &str, extra: &[char]) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.chars().all(|character| character.is_ascii_alphanumeric() || extra.contains(&character))
}

pub(crate) struct Local {
    pub(crate) user: String,
    pub(crate) host: String,
    pub(crate) home: String,
    pub(crate) uid: String,
}

impl Local {
    pub(crate) fn detect(user: &str) -> Self {
        Self {
            user: user.to_string(),
            host: gethostname::gethostname().to_string_lossy().into_owned(),
            home: porthmos_vfs::Environment::from_process()
                .home
                .map(|home| home.to_string_lossy().into_owned())
                .unwrap_or_default(),
            uid: local_uid(),
        }
    }
}

#[cfg(unix)]
fn local_uid() -> String {
    unsafe { libc::getuid() }.to_string()
}

#[cfg(not(unix))]
fn local_uid() -> String {
    String::new()
}

pub(crate) struct Tokens<'a> {
    pub(crate) host: &'a str,
    pub(crate) port: u16,
    pub(crate) user: &'a str,
    pub(crate) name: &'a str,
    pub(crate) local: &'a Local,
}

const NAME_CHARACTERS: &[char] = &['.', '-', '_'];
const HOST_CHARACTERS: &[char] = &['.', '-', '_', ':'];
const PATH_CHARACTERS: &[char] = &['.', '-', '_', '/', '\\', ':', '~'];

fn checked<'a>(value: &'a str, extra: &[char], what: &str, token: char) -> Result<&'a str, String> {
    if value.is_empty() {
        Err(format!("the {what} is not known here, so %{token} cannot be used"))
    } else if safe_in_a_command(value, extra) {
        Ok(value)
    } else {
        Err(format!("the {what} has characters the shell would treat as code, so %{token} cannot be used"))
    }
}

fn connection_hash(tokens: &Tokens) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(tokens.local.host.as_bytes());
    hasher.update(tokens.host.as_bytes());
    hasher.update(tokens.port.to_string().as_bytes());
    hasher.update(tokens.user.as_bytes());
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn expand(command: &str, tokens: &Tokens) -> Result<String, String> {
    let mut expanded = String::with_capacity(command.len());
    let mut characters = command.chars();
    while let Some(character) = characters.next() {
        if character != '%' {
            expanded.push(character);
            continue;
        }
        let local = tokens.local;
        match characters.next() {
            Some('%') => expanded.push('%'),
            Some('p') => expanded.push_str(&tokens.port.to_string()),
            Some('h') => expanded.push_str(checked(tokens.host, HOST_CHARACTERS, "host name", 'h')?),
            Some('k') => expanded.push_str(checked(tokens.host, HOST_CHARACTERS, "host name", 'k')?),
            Some('r') => expanded.push_str(checked(tokens.user, NAME_CHARACTERS, "user name", 'r')?),
            Some('n') => expanded.push_str(checked(tokens.name, NAME_CHARACTERS, "name", 'n')?),
            Some('u') => expanded.push_str(checked(&local.user, NAME_CHARACTERS, "local user name", 'u')?),
            Some('l') => expanded.push_str(checked(&local.host, HOST_CHARACTERS, "local host name", 'l')?),
            Some('L') => {
                let full = checked(&local.host, HOST_CHARACTERS, "local host name", 'L')?;
                expanded.push_str(full.split('.').next().unwrap_or(full));
            }
            Some('d') => expanded.push_str(checked(&local.home, PATH_CHARACTERS, "home folder", 'd')?),
            Some('i') => expanded.push_str(checked(&local.uid, &[], "local user ID", 'i')?),
            Some('j') => {}
            Some('C') => expanded.push_str(&connection_hash(tokens)),
            Some(other) => return Err(format!("the proxy command uses %{other}, which is not known")),
            None => return Err("the proxy command ends with a lone %".to_string()),
        }
    }
    Ok(expanded)
}

pub(crate) struct CommandStream {
    io: Join<ChildStdout, ChildStdin>,
    _child: Child,
    waiting_for_first_byte: Option<Pin<Box<Sleep>>>,
}

impl AsyncRead for CommandStream {
    fn poll_read(
        mut self: Pin<&mut Self>, context: &mut Context<'_>, buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = &mut *self;
        let before = buffer.filled().len();
        match Pin::new(&mut this.io).poll_read(context, buffer) {
            Poll::Ready(Ok(())) => {
                if buffer.filled().len() > before {
                    this.waiting_for_first_byte = None;
                }
                Poll::Ready(Ok(()))
            }
            Poll::Pending => {
                if let Some(waiting) = this.waiting_for_first_byte.as_mut()
                    && waiting.as_mut().poll(context).is_ready()
                {
                    this.waiting_for_first_byte = None;
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "the proxy command sent nothing in time",
                    )));
                }
                Poll::Pending
            }
            other => other,
        }
    }
}

impl AsyncWrite for CommandStream {
    fn poll_write(mut self: Pin<&mut Self>, context: &mut Context<'_>, data: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(context, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(context)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(context)
    }
}

#[derive(Clone)]
pub(crate) struct Diagnostics {
    text: Arc<Mutex<String>>,
    finished: watch::Receiver<bool>,
}

impl Diagnostics {
    pub(crate) async fn text(&self) -> String {
        let mut finished = self.finished.clone();
        let _ = tokio::time::timeout(DIAGNOSTIC_WAIT, finished.wait_for(|done| *done)).await;
        let raw = self.text.lock().map(|text| text.clone()).unwrap_or_default();
        let mut cleaned = String::new();
        for character in raw.trim().chars() {
            cleaned.push(if character.is_control() { ' ' } else { character });
            if cleaned.chars().count() >= MAX_DIAGNOSTIC_CHARS {
                break;
            }
        }
        cleaned.trim().to_string()
    }
}

fn shell(command: &str) -> Command {
    #[cfg(unix)]
    {
        let mut process = Command::new("sh");
        process.arg("-c").arg(command);
        process
    }
    #[cfg(windows)]
    {
        let mut process = Command::new("cmd");
        process.arg("/C").raw_arg(command);
        process.creation_flags(CREATE_NO_WINDOW);
        process
    }
}

async fn collect(mut stderr: ChildStderr, text: Arc<Mutex<String>>, finished: watch::Sender<bool>) {
    let mut kept = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        match stderr.read(&mut chunk).await {
            Ok(count) if count > 0 => {
                if kept.len() < MAX_STDERR_BYTES {
                    kept.extend_from_slice(&chunk[..count.min(MAX_STDERR_BYTES - kept.len())]);
                    if let Ok(mut text) = text.lock() {
                        *text = String::from_utf8_lossy(&kept).into_owned();
                    }
                }
            }
            _ => break,
        }
    }
    let _ = finished.send(true);
}

pub(crate) fn spawn(command: &str) -> io::Result<(CommandStream, Diagnostics)> {
    spawn_with(command, FIRST_BYTE_TIME)
}

pub(crate) fn spawn_with(command: &str, first_byte_within: Duration) -> io::Result<(CommandStream, Diagnostics)> {
    let mut process = shell(command);
    process.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut child = process.spawn()?;
    let missing = || io::Error::other("the proxy command has no standard input or output");
    let stdin = child.stdin.take().ok_or_else(missing)?;
    let stdout = child.stdout.take().ok_or_else(missing)?;
    let stderr = child.stderr.take().ok_or_else(missing)?;
    let text = Arc::new(Mutex::new(String::new()));
    let (done, finished) = watch::channel(false);
    tokio::spawn(collect(stderr, text.clone(), done));
    let stream = CommandStream {
        io: tokio::io::join(stdout, stdin),
        _child: child,
        waiting_for_first_byte: Some(Box::pin(tokio::time::sleep(first_byte_within))),
    };
    Ok((stream, Diagnostics { text, finished }))
}

#[cfg(test)]
mod tests;
