use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::oneshot,
};

use crate::TermSize;

pub type ResizeFn = Box<dyn Fn(TermSize) + Send + Sync>;

pub struct Transport {
    pub reader: Box<dyn AsyncRead + Send + Unpin>,
    pub writer: Box<dyn AsyncWrite + Send + Unpin>,
    pub resize: ResizeFn,
    pub exit: oneshot::Receiver<Option<u32>>,
}
