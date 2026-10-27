use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{DirItem, Entry, ErrorKind, FileKind, FileSystem, Metadata, ProtocolError, Reader, VfsPath, Writer};

#[derive(Debug, Clone)]
enum Node {
    File { data: Vec<u8>, modified: Option<u64> },
    Dir,
    Symlink(VfsPath),
}

#[derive(Default)]
struct Tree {
    nodes: BTreeMap<VfsPath, Node>,
    failing_read_dirs: BTreeSet<VfsPath>,
    failing_reads: BTreeSet<VfsPath>,
    unopenable: BTreeSet<VfsPath>,
    hanging_reads: BTreeMap<VfsPath, usize>,
    failing_home: bool,
    free_space: Option<u64>,
    failing_free_space: bool,
    hanging_free_space: bool,
    connection_lost: bool,
    owners: BTreeMap<VfsPath, (String, String)>,
    denied_stats: BTreeSet<VfsPath>,
    hanging_stats: BTreeSet<VfsPath>,
    server_info: Option<String>,
    failing_shutdowns: BTreeSet<VfsPath>,
    written_sizes: Vec<(VfsPath, u64)>,
}

const MAX_SYMLINK_HOPS: usize = 8;

impl Tree {
    fn insert_ancestors(&mut self, path: &VfsPath) {
        for ancestor in path.ancestors().skip(1) {
            self.nodes.entry(ancestor.clone()).or_insert(Node::Dir);
        }
    }

    fn resolve(&self, path: &VfsPath) -> Option<&Node> {
        let mut current = path.clone();
        for _ in 0..MAX_SYMLINK_HOPS {
            match self.nodes.get(&current)? {
                Node::Symlink(target) => current = target.clone(),
                node => return Some(node),
            }
        }
        None
    }

    fn children(&self, dir: &VfsPath) -> Vec<(VfsPath, &Node)> {
        self.nodes
            .iter()
            .filter(|(path, _)| path.parent().as_ref() == Some(dir))
            .map(|(path, node)| (path.clone(), node))
            .collect()
    }

    fn is_dir(&self, path: &VfsPath) -> bool {
        matches!(self.nodes.get(path), Some(Node::Dir))
    }
}

fn metadata_of(node: &Node) -> Metadata {
    match node {
        Node::File { data, modified } => {
            Metadata { size: data.len() as u64, modified: *modified, kind: FileKind::File, permissions: None }
        }
        Node::Dir => Metadata { size: 0, modified: None, kind: FileKind::Dir, permissions: None },
        Node::Symlink(_) => Metadata { size: 0, modified: None, kind: FileKind::Symlink, permissions: None },
    }
}

fn not_found(path: &VfsPath) -> ProtocolError {
    ProtocolError::new(ErrorKind::NotFound, anyhow::anyhow!("No such file: {}", path))
}

fn name_of(path: &VfsPath) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

#[derive(Clone)]
pub struct FakeFs {
    tree: Arc<Mutex<Tree>>,
    resume_backoff: u64,
    home: VfsPath,
    time_support: bool,
    time_resolution: u64,
    holds_connection: bool,
}

impl Default for FakeFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeFs {
    pub fn new() -> Self {
        let fs = Self {
            tree: Arc::default(),
            resume_backoff: 0,
            home: VfsPath::new("/home/user"),
            time_support: true,
            time_resolution: 2,
            holds_connection: false,
        };
        fs.dir("/home/user");
        fs
    }

    pub fn denying_stat(&self, path: impl Into<VfsPath>) -> &Self {
        self.tree.lock().unwrap().denied_stats.insert(path.into());
        self
    }

    pub fn hanging_stat(&self, path: impl Into<VfsPath>) -> &Self {
        self.tree.lock().unwrap().hanging_stats.insert(path.into());
        self
    }

    pub fn handles(&self) -> usize {
        Arc::strong_count(&self.tree)
    }

    pub fn with_server_info(&self, text: &str) -> &Self {
        self.tree.lock().unwrap().server_info = Some(text.to_string());
        self
    }

    pub fn with_free_space(&self, bytes: u64) -> &Self {
        self.tree.lock().unwrap().free_space = Some(bytes);
        self
    }

    pub fn fail_free_space(&self) -> &Self {
        self.tree.lock().unwrap().failing_free_space = true;
        self
    }

    pub fn holding_connection(mut self) -> Self {
        self.holds_connection = true;
        self
    }

    pub fn lose_connection(&self) -> &Self {
        self.tree.lock().unwrap().connection_lost = true;
        self
    }

    pub fn restore_connection(&self) -> &Self {
        self.tree.lock().unwrap().connection_lost = false;
        self
    }

    pub fn hanging_free_space(&self) -> &Self {
        self.tree.lock().unwrap().hanging_free_space = true;
        self
    }

    pub fn with_owner(&self, path: impl Into<VfsPath>, owner: &str, group: &str) -> &Self {
        self.tree.lock().unwrap().owners.insert(path.into(), (owner.to_string(), group.to_string()));
        self
    }

    pub fn dir(&self, path: impl Into<VfsPath>) -> &Self {
        let path = path.into();
        let mut tree = self.tree.lock().unwrap();
        tree.insert_ancestors(&path);
        tree.nodes.insert(path, Node::Dir);
        self
    }

    pub fn file(&self, path: impl Into<VfsPath>, data: &[u8], modified: Option<u64>) -> &Self {
        let path = path.into();
        let mut tree = self.tree.lock().unwrap();
        tree.insert_ancestors(&path);
        tree.nodes.insert(path, Node::File { data: data.to_vec(), modified });
        self
    }

    pub fn symlink(&self, path: impl Into<VfsPath>, target: impl Into<VfsPath>) -> &Self {
        let path = path.into();
        let mut tree = self.tree.lock().unwrap();
        tree.insert_ancestors(&path);
        tree.nodes.insert(path, Node::Symlink(target.into()));
        self
    }

    pub fn fail_read_dir(&self, path: impl Into<VfsPath>) -> &Self {
        self.tree.lock().unwrap().failing_read_dirs.insert(path.into());
        self
    }

    pub fn hanging_reads(&self, path: impl Into<VfsPath>, after: usize) -> &Self {
        self.tree.lock().unwrap().hanging_reads.insert(path.into(), after);
        self
    }

    pub fn deny_open(&self, path: impl Into<VfsPath>) -> &Self {
        self.tree.lock().unwrap().unopenable.insert(path.into());
        self
    }

    pub fn fail_reads(&self, path: impl Into<VfsPath>) -> &Self {
        self.tree.lock().unwrap().failing_reads.insert(path.into());
        self
    }

    pub fn written_sizes(&self) -> Vec<(VfsPath, u64)> {
        self.tree.lock().unwrap().written_sizes.clone()
    }

    pub fn fail_shutdown(&self, path: impl Into<VfsPath>) -> &Self {
        self.tree.lock().unwrap().failing_shutdowns.insert(path.into());
        self
    }

    pub fn fail_home(&self) -> &Self {
        self.tree.lock().unwrap().failing_home = true;
        self
    }

    pub fn with_resume_backoff(mut self, bytes: u64) -> Self {
        self.resume_backoff = bytes;
        self
    }

    pub fn with_time_support(mut self, supported: bool) -> Self {
        self.time_support = supported;
        self
    }

    pub fn with_time_resolution(mut self, seconds: u64) -> Self {
        self.time_resolution = seconds;
        self
    }

    pub fn modified_of(&self, path: impl Into<VfsPath>) -> Option<u64> {
        match self.tree.lock().unwrap().nodes.get(&path.into() as &VfsPath) {
            Some(Node::File { modified, .. }) => *modified,
            _ => None,
        }
    }

    pub fn with_home(mut self, home: impl Into<VfsPath>) -> Self {
        let home = home.into();
        self.dir(&home);
        self.home = home;
        self
    }

    pub fn contents(&self, path: impl Into<VfsPath>) -> Option<Vec<u8>> {
        match self.tree.lock().unwrap().nodes.get(&path.into() as &VfsPath) {
            Some(Node::File { data, .. }) => Some(data.clone()),
            _ => None,
        }
    }

    pub fn exists(&self, path: impl Into<VfsPath>) -> bool {
        self.tree.lock().unwrap().nodes.contains_key(&path.into() as &VfsPath)
    }
}

struct HangingReader {
    data: std::io::Cursor<Vec<u8>>,
}

impl AsyncRead for HangingReader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        if self.data.position() < self.data.get_ref().len() as u64 {
            return Pin::new(&mut self.data).poll_read(cx, buf);
        }
        Poll::Pending
    }
}

struct FailingReader;

impl AsyncRead for FailingReader {
    fn poll_read(self: Pin<&mut Self>, _cx: &mut Context<'_>, _buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::other("injected read failure")))
    }
}

struct FakeWriter {
    tree: Arc<Mutex<Tree>>,
    path: VfsPath,
}

impl AsyncWrite for FakeWriter {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let mut tree = self.tree.lock().unwrap();
        match tree.nodes.get_mut(&self.path) {
            Some(Node::File { data, .. }) => {
                data.extend_from_slice(buf);
                Poll::Ready(Ok(buf.len()))
            }
            _ => Poll::Ready(Err(io::Error::from(io::ErrorKind::NotFound))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.tree.lock().unwrap().failing_shutdowns.contains(&self.path) {
            return Poll::Ready(Err(io::Error::other("shutdown refused")));
        }
        Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl FileSystem for FakeFs {
    async fn set_modified(&self, path: &VfsPath, seconds: u64) -> Result<(), ProtocolError> {
        if !self.time_support {
            return Err(ProtocolError::new(
                ErrorKind::Unsupported,
                anyhow::anyhow!("this connection can't set modification times"),
            ));
        }
        let mut tree = self.tree.lock().unwrap();
        match tree.nodes.get_mut(path) {
            Some(Node::File { modified, .. }) => {
                *modified = Some(seconds);
                Ok(())
            }
            _ => Err(not_found(path)),
        }
    }

    fn can_set_modified(&self) -> bool {
        self.time_support
    }

    fn server_info(&self) -> Option<String> {
        self.tree.lock().unwrap().server_info.clone()
    }

    fn time_resolution(&self) -> u64 {
        self.time_resolution
    }

    async fn list(&self, dir: &VfsPath) -> Result<Vec<Entry>, ProtocolError> {
        let tree = self.tree.lock().unwrap();
        if !matches!(tree.resolve(dir), Some(Node::Dir)) {
            return Err(not_found(dir));
        }
        Ok(tree
            .children(dir)
            .into_iter()
            .map(|(path, node)| {
                let resolved = tree.resolve(&path).map(metadata_of).unwrap_or_else(|| metadata_of(node));
                let (owner, group) = match tree.owners.get(&path) {
                    Some((owner, group)) => (Some(owner.clone()), Some(group.clone())),
                    None => (None, None),
                };
                let link_target = match node {
                    Node::Symlink(target) => Some(target.to_string_lossy().into_owned()),
                    _ => None,
                };
                Entry {
                    name: name_of(&path),
                    is_dir: resolved.is_dir(),
                    size: resolved.size,
                    permissions: None,
                    modified: resolved.modified,
                    path,
                    owner,
                    group,
                    link_target,
                }
            })
            .collect())
    }

    async fn read_dir(&self, dir: &VfsPath) -> Result<Vec<DirItem>, ProtocolError> {
        let tree = self.tree.lock().unwrap();
        if tree.failing_read_dirs.contains(dir) {
            return Err(ProtocolError::new(ErrorKind::PermissionDenied, anyhow::anyhow!("Permission denied")));
        }
        if !tree.is_dir(dir) {
            return Err(not_found(dir));
        }
        Ok(tree
            .children(dir)
            .into_iter()
            .map(|(path, node)| DirItem { name: name_of(&path), metadata: metadata_of(node), path })
            .collect())
    }

    async fn stat(&self, path: &VfsPath) -> Result<Metadata, ProtocolError> {
        let hangs = self.tree.lock().unwrap().hanging_stats.contains(path);
        if hangs {
            std::future::pending::<()>().await;
        }
        let tree = self.tree.lock().unwrap();
        if tree.denied_stats.contains(path) {
            return Err(ProtocolError::new(ErrorKind::PermissionDenied, anyhow::anyhow!("permission denied")));
        }
        tree.resolve(path).map(metadata_of).ok_or_else(|| not_found(path))
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let mut tree = self.tree.lock().unwrap();
        if !path.parent().is_some_and(|parent| tree.is_dir(&parent)) {
            return Err(not_found(path));
        }
        if tree.nodes.contains_key(path) {
            return Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("File exists")));
        }
        tree.nodes.insert(path.clone(), Node::Dir);
        Ok(())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), ProtocolError> {
        let mut tree = self.tree.lock().unwrap();
        if !tree.nodes.contains_key(from) {
            return Err(not_found(from));
        }
        let moved: Vec<(VfsPath, Node)> = tree
            .nodes
            .iter()
            .filter(|(path, _)| path.starts_with(from))
            .map(|(path, node)| (to.join(path.strip_prefix(from).unwrap_or_else(|| path.clone())), node.clone()))
            .collect();
        tree.nodes.retain(|path, _| !path.starts_with(from) && !path.starts_with(to));
        tree.nodes.extend(moved);
        Ok(())
    }

    async fn remove_file(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let mut tree = self.tree.lock().unwrap();
        match tree.nodes.get(path) {
            Some(Node::File { .. } | Node::Symlink(_)) => {
                tree.nodes.remove(path);
                Ok(())
            }
            Some(Node::Dir) => Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("Is a directory"))),
            None => Err(not_found(path)),
        }
    }

    async fn remove_dir(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let mut tree = self.tree.lock().unwrap();
        match tree.nodes.get(path) {
            None => Err(not_found(path)),
            Some(Node::Dir) if !tree.children(path).is_empty() => {
                Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("{} is not empty", path)))
            }
            Some(Node::Dir) => {
                tree.nodes.remove(path);
                Ok(())
            }
            Some(_) => Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("{} is not a folder", path))),
        }
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let mut tree = self.tree.lock().unwrap();
        if !tree.nodes.contains_key(path) {
            return Err(not_found(path));
        }
        tree.nodes.retain(|candidate, _| !candidate.starts_with(path));
        Ok(())
    }

    async fn connection_alive(&self) -> Option<bool> {
        self.holds_connection.then(|| !self.tree.lock().unwrap().connection_lost)
    }

    async fn free_space(&self, _dir: &VfsPath) -> Result<Option<u64>, ProtocolError> {
        let hangs = self.tree.lock().unwrap().hanging_free_space;
        if hangs {
            std::future::pending::<()>().await;
        }
        let tree = self.tree.lock().unwrap();
        if tree.failing_free_space {
            return Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("free space unavailable")));
        }
        Ok(tree.free_space)
    }

    async fn home(&self) -> Result<VfsPath, ProtocolError> {
        if self.tree.lock().unwrap().failing_home {
            return Err(ProtocolError::new(ErrorKind::PermissionDenied, anyhow::anyhow!("Permission denied")));
        }
        Ok(self.home.clone())
    }

    async fn open_read(&self, path: &VfsPath, offset: u64) -> Result<Reader, ProtocolError> {
        let tree = self.tree.lock().unwrap();
        if tree.unopenable.contains(path) {
            return Err(ProtocolError::new(ErrorKind::PermissionDenied, anyhow::anyhow!("Permission denied")));
        }
        if tree.failing_reads.contains(path) {
            return Ok(Box::new(FailingReader));
        }
        match tree.resolve(path) {
            Some(Node::File { data, .. }) => {
                let start = (offset as usize).min(data.len());
                if let Some(after) = tree.hanging_reads.get(path) {
                    let end = (start + after).min(data.len());
                    return Ok(Box::new(HangingReader { data: std::io::Cursor::new(data[start..end].to_vec()) }));
                }
                Ok(Box::new(std::io::Cursor::new(data[start..].to_vec())))
            }
            _ => Err(not_found(path)),
        }
    }

    async fn open_write_sized(&self, path: &VfsPath, offset: u64, size: u64) -> Result<Writer, ProtocolError> {
        self.tree.lock().unwrap().written_sizes.push((path.clone(), size));
        self.open_write(path, offset).await
    }

    async fn open_write(&self, path: &VfsPath, offset: u64) -> Result<Writer, ProtocolError> {
        let mut tree = self.tree.lock().unwrap();
        if !path.parent().is_some_and(|parent| tree.is_dir(&parent)) {
            return Err(not_found(path));
        }
        let start = match tree.nodes.get_mut(path) {
            Some(Node::File { data, .. }) if offset > 0 => {
                data.resize(offset as usize, 0);
                offset
            }
            _ => {
                tree.nodes.insert(path.clone(), Node::File { data: Vec::new(), modified: None });
                0
            }
        };
        Ok(Writer { stream: Box::new(FakeWriter { tree: self.tree.clone(), path: path.clone() }), offset: start })
    }

    fn resume_backoff(&self) -> u64 {
        self.resume_backoff
    }
}
