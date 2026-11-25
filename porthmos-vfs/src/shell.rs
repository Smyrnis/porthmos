use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::oneshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellSize {
    pub columns: u16,
    pub rows: u16,
}

pub struct ShellStream {
    pub reader: Box<dyn AsyncRead + Send + Unpin>,
    pub writer: Box<dyn AsyncWrite + Send + Unpin>,
    pub resize: Box<dyn Fn(ShellSize) + Send + Sync>,
    pub exit: oneshot::Receiver<Option<u32>>,
}
