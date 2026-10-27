use std::sync::{Arc, atomic::AtomicBool};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    DirItem, Entry, ErrorKind, Metadata, ProtocolError, SearchQuery, SearchSender, VfsPath, search::walk_search,
};

pub type Reader = Box<dyn AsyncRead + Send + Unpin>;

pub struct Writer {
    pub stream: Box<dyn AsyncWrite + Send + Unpin>,
    pub offset: u64,
}

#[async_trait]
pub trait FileSystem: Send + Sync {
    async fn list(&self, dir: &VfsPath) -> Result<Vec<Entry>, ProtocolError>;
    async fn read_dir(&self, dir: &VfsPath) -> Result<Vec<DirItem>, ProtocolError>;
    async fn stat(&self, path: &VfsPath) -> Result<Metadata, ProtocolError>;
    async fn create_dir(&self, path: &VfsPath) -> Result<(), ProtocolError>;
    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), ProtocolError>;
    async fn remove_file(&self, path: &VfsPath) -> Result<(), ProtocolError>;
    async fn delete(&self, path: &VfsPath) -> Result<(), ProtocolError>;
    async fn remove_dir(&self, _path: &VfsPath) -> Result<(), ProtocolError> {
        Err(ProtocolError::new(
            ErrorKind::Unsupported,
            anyhow::anyhow!("this connection can't remove a folder without deleting what is inside it"),
        ))
    }
    async fn home(&self) -> Result<VfsPath, ProtocolError>;
    async fn open_read(&self, path: &VfsPath, offset: u64) -> Result<Reader, ProtocolError>;
    async fn open_write(&self, path: &VfsPath, offset: u64) -> Result<Writer, ProtocolError>;
    async fn open_write_sized(&self, path: &VfsPath, offset: u64, _size: u64) -> Result<Writer, ProtocolError> {
        self.open_write(path, offset).await
    }
    async fn connection_alive(&self) -> Option<bool> {
        None
    }
    fn transfer_limit(&self) -> Option<usize> {
        None
    }
    fn resume_backoff(&self) -> u64 {
        0
    }
    fn server_info(&self) -> Option<String> {
        None
    }
    async fn set_modified(&self, _path: &VfsPath, _seconds: u64) -> Result<(), ProtocolError> {
        Err(ProtocolError::new(ErrorKind::Unsupported, anyhow::anyhow!("this connection can't set modification times")))
    }

    fn can_set_modified(&self) -> bool {
        false
    }

    fn time_resolution(&self) -> u64 {
        2
    }
    async fn free_space(&self, _dir: &VfsPath) -> Result<Option<u64>, ProtocolError> {
        Ok(None)
    }
    async fn search(&self, query: SearchQuery, tx: SearchSender, cancel: Arc<AtomicBool>) {
        walk_search(self, query, tx, cancel).await
    }
}
