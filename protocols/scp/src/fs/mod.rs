use std::{
    io,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use porthmos_ssh::{ExecChannel, Output, Session, exec, open_exec};
use porthmos_vfs::{
    DirItem, Entry, ErrorKind, FileSystem, Metadata, ProtocolError, Reader, VfsPath, Writer, async_trait,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    commands::{self, failure},
    listing::{self, Line},
    streams::{ChannelReader, ChannelWriter, Finisher, Pump, checked_exit, read_line, read_reply, within},
    wire::parse_header,
};

const TRANSFER_CHANNELS: usize = 6;
const COMMAND_CHANNELS: usize = 2;
const FINAL_WAIT_FACTOR: u32 = 20;

pub(crate) struct ScpFs {
    session: Arc<Session>,
    home: VfsPath,
    scp: bool,
    timeout: Duration,
    transfers: Arc<Semaphore>,
    commands: Arc<Semaphore>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or_default()
}

fn other(err: io::Error) -> ProtocolError {
    match err.into_inner() {
        Some(inner) => match inner.downcast::<ProtocolError>() {
            Ok(protocol) => *protocol,
            Err(inner) => ProtocolError::new(ErrorKind::Other, anyhow::anyhow!(inner.to_string())),
        },
        None => ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("the transfer failed")),
    }
}

fn as_protocol(err: io::Error) -> ProtocolError {
    if err.kind() == io::ErrorKind::TimedOut {
        return ProtocolError::new(ErrorKind::Other, anyhow::anyhow!(err.to_string()));
    }
    other(err)
}

impl ScpFs {
    pub(crate) fn new(session: Arc<Session>, home: VfsPath, scp: bool, timeout: Duration) -> Self {
        Self {
            session,
            home,
            scp,
            timeout,
            transfers: Arc::new(Semaphore::new(TRANSFER_CHANNELS)),
            commands: Arc::new(Semaphore::new(COMMAND_CHANNELS)),
        }
    }

    fn final_wait(&self) -> Duration {
        self.timeout * FINAL_WAIT_FACTOR
    }

    async fn permit(pool: &Arc<Semaphore>) -> OwnedSemaphorePermit {
        pool.clone().acquire_owned().await.expect("the channel semaphores are never closed")
    }

    async fn run(&self, command: &str) -> Result<Output, ProtocolError> {
        let _permit = Self::permit(&self.commands).await;
        match tokio::time::timeout(self.final_wait(), exec(&self.session, command)).await {
            Ok(result) => result,
            Err(_) => Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!(crate::streams::NO_RESPONSE))),
        }
    }

    async fn run_ok(&self, command: &str, path: &VfsPath) -> Result<Output, ProtocolError> {
        let output = self.run(command).await?;
        if output.status == Some(0) {
            Ok(output)
        } else {
            Err(failure(&output.stderr, output.status, path, command.split(' ').next().unwrap_or_default()))
        }
    }

    async fn lines(&self, command: &str, path: &VfsPath) -> Result<Vec<Line>, ProtocolError> {
        let output = self.run(command).await?;
        if output.status != Some(0) && output.stdout.is_empty() {
            return Err(failure(&output.stderr, output.status, path, "ls"));
        }
        Ok(listing::parse(&String::from_utf8_lossy(&output.stdout), now()))
    }

    async fn channel(&self, command: &str) -> Result<(ExecChannel, OwnedSemaphorePermit), ProtocolError> {
        let permit = Self::permit(&self.transfers).await;
        let channel = match tokio::time::timeout(self.timeout, open_exec(&self.session, command)).await {
            Ok(opened) => opened?,
            Err(_) => return Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!(crate::streams::NO_RESPONSE))),
        };
        Ok((channel, permit))
    }

    fn exit_finisher(&self, path: &VfsPath, command: &str) -> Finisher {
        let (path, command, wait) = (path.clone(), command.to_string(), self.final_wait());
        Box::new(move |channel| {
            Box::pin(async move { within(wait, async { checked_exit(channel.finish().await, &path, &command) }).await })
        })
    }

    fn source_finisher(&self, path: &VfsPath) -> Finisher {
        let (path, wait) = (path.clone(), self.final_wait());
        Box::new(move |mut channel| {
            Box::pin(async move {
                within(wait, async {
                    read_reply(&mut channel.output, &path).await?;
                    channel.input.send(&[0]).await?;
                    channel.input.close().await?;
                    checked_exit(channel.finish().await, &path, "scp")
                })
                .await
            })
        })
    }

    async fn scp_source(&self, path: &VfsPath) -> Result<Reader, ProtocolError> {
        let (mut channel, permit) = self.channel(&commands::scp_source(path)).await?;
        let size = within(self.timeout, async {
            channel.input.send(&[0]).await?;
            loop {
                let line = read_line(&mut channel.output).await?;
                match line.first() {
                    Some(b'T') => channel.input.send(&[0]).await?,
                    Some(1 | 2) => return Err(io::Error::other(failure(&line[1..], None, path, "scp"))),
                    _ => {
                        let header = parse_header(&line).map_err(io::Error::other)?;
                        channel.input.send(&[0]).await?;
                        return Ok(header.size);
                    }
                }
            }
        })
        .await
        .map_err(as_protocol)?;
        let reader = ChannelReader::new(channel, Some(size), self.source_finisher(path), self.timeout, permit);
        Ok(Box::new(reader))
    }

    async fn command_reader(&self, path: &VfsPath, offset: u64) -> Result<Reader, ProtocolError> {
        let command = commands::read_from(path, offset);
        let (channel, permit) = self.channel(&command).await?;
        channel.input.close().await.map_err(as_protocol)?;
        let name = command.split(' ').next().unwrap_or_default().to_string();
        Ok(Box::new(ChannelReader::new(channel, None, self.exit_finisher(path, &name), self.timeout, permit)))
    }

    async fn scp_sink(&self, path: &VfsPath, size: u64) -> Result<Writer, ProtocolError> {
        let (mut channel, permit) = self.channel(&commands::scp_sink(path)).await?;
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let ready = within(self.timeout, async {
            read_reply(&mut channel.output, path).await?;
            channel.input.send(crate::wire::sink_header(size, &name).as_bytes()).await?;
            read_reply(&mut channel.output, path).await
        })
        .await;
        if let Err(err) = ready {
            let output = within(self.timeout, async { Ok(channel.finish().await) }).await.unwrap_or_default();
            if output.status.is_some_and(|status| status != 0) && !output.stderr.is_empty() {
                return Err(failure(&output.stderr, output.status, path, "scp"));
            }
            return Err(as_protocol(err));
        }
        let job =
            Pump { channel, expected: Some(size), path: path.clone(), command: "scp".to_string(), idle: self.timeout };
        Ok(Writer { stream: Box::new(ChannelWriter::new(job, self.final_wait(), permit)), offset: 0 })
    }

    async fn command_writer(&self, path: &VfsPath, offset: u64) -> Result<Writer, ProtocolError> {
        let command = if offset > 0 { commands::append(path) } else { commands::create(path) };
        let (channel, permit) = self.channel(&command).await?;
        let job = Pump { channel, expected: None, path: path.clone(), command: "cat".to_string(), idle: self.timeout };
        Ok(Writer { stream: Box::new(ChannelWriter::new(job, self.final_wait(), permit)), offset })
    }
}

#[async_trait]
impl FileSystem for ScpFs {
    fn server_info(&self) -> Option<String> {
        self.session.server_version()
    }

    async fn connection_alive(&self) -> Option<bool> {
        Some(!self.session.is_closed())
    }

    async fn list(&self, dir: &VfsPath) -> Result<Vec<Entry>, ProtocolError> {
        let lines = self.lines(&commands::list(dir), dir).await?;
        Ok(lines
            .into_iter()
            .map(|line| Entry {
                is_dir: line.metadata.is_dir(),
                size: line.metadata.size,
                permissions: line.metadata.permissions,
                modified: line.metadata.modified,
                path: dir.join(&line.name),
                name: line.name,
                owner: Some(line.owner),
                group: Some(line.group),
                link_target: None,
                hidden: false,
            })
            .collect())
    }

    async fn free_space(&self, dir: &VfsPath) -> Result<Option<u64>, ProtocolError> {
        let output = self.run(&commands::free_space(dir)).await?;
        if output.status != Some(0) && output.stdout.is_empty() {
            return Err(failure(&output.stderr, output.status, dir, "df"));
        }
        Ok(listing::parse_free_space(&String::from_utf8_lossy(&output.stdout)))
    }

    async fn read_dir(&self, dir: &VfsPath) -> Result<Vec<DirItem>, ProtocolError> {
        let lines = self.lines(&commands::list(dir), dir).await?;
        Ok(lines
            .into_iter()
            .map(|line| DirItem { path: dir.join(&line.name), name: line.name, metadata: line.metadata })
            .collect())
    }

    async fn stat(&self, path: &VfsPath) -> Result<Metadata, ProtocolError> {
        let lines = self.lines(&commands::stat(path), path).await?;
        lines
            .into_iter()
            .next()
            .map(|line| line.metadata)
            .ok_or_else(|| ProtocolError::new(ErrorKind::NotFound, anyhow::anyhow!("{} not found", path)))
    }

    async fn set_modified(&self, path: &VfsPath, seconds: u64) -> Result<(), ProtocolError> {
        let output = self.run(&commands::touch(path, seconds)).await?;
        match output.status {
            Some(0) => Ok(()),
            Some(1) if output.stderr.is_empty() => {
                Err(ProtocolError::new(ErrorKind::NotFound, anyhow::anyhow!("{} not found", path)))
            }
            status => Err(failure(&output.stderr, status, path, "touch")),
        }
    }

    fn can_set_modified(&self) -> bool {
        true
    }

    fn time_resolution(&self) -> u64 {
        60
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        self.run_ok(&commands::mkdir(path), path).await.map(|_| ())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), ProtocolError> {
        match self.stat(to).await {
            Ok(metadata) if metadata.is_dir() => {
                return Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("{} already exists", to)));
            }
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        match self.run_ok(&commands::rename(from, to), from).await {
            Err(err) if err.kind() == ErrorKind::NotFound && self.stat(from).await.is_ok() => {
                Err(ProtocolError::new(ErrorKind::NotFound, anyhow::anyhow!("{} not found", to)))
            }
            result => result.map(|_| ()),
        }
    }

    async fn remove_file(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        self.run_ok(&commands::remove(path), path).await.map(|_| ())
    }

    async fn remove_dir(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        self.run_ok(&commands::remove_dir(path), path).await.map(|_| ())
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        self.stat(path).await?;
        self.run_ok(&commands::remove_tree(path), path).await.map(|_| ())
    }

    fn transfer_limit(&self) -> Option<usize> {
        Some(TRANSFER_CHANNELS)
    }

    async fn home(&self) -> Result<VfsPath, ProtocolError> {
        Ok(self.home.clone())
    }

    async fn open_read(&self, path: &VfsPath, offset: u64) -> Result<Reader, ProtocolError> {
        if offset == 0 && self.scp {
            return self.scp_source(path).await;
        }
        self.command_reader(path, offset).await
    }

    async fn open_write(&self, path: &VfsPath, offset: u64) -> Result<Writer, ProtocolError> {
        self.command_writer(path, offset).await
    }

    async fn open_write_sized(&self, path: &VfsPath, offset: u64, size: u64) -> Result<Writer, ProtocolError> {
        if offset == 0 && self.scp {
            return self.scp_sink(path, size).await;
        }
        self.command_writer(path, offset).await
    }
}
