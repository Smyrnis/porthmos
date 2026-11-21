use std::sync::Arc;

use porthmos_vfs::{Environment, FileSystem, Protocol};

use crate::Paths;

pub(crate) struct Host {
    pub(crate) paths: Paths,
    pub(crate) env: Environment,
    pub(crate) protocols: Vec<Arc<dyn Protocol>>,
    pub(crate) local_fs: Arc<dyn FileSystem>,
    pub(crate) clock: fn() -> chrono::DateTime<chrono::Utc>,
    pub(crate) local_offset: fn() -> chrono::FixedOffset,
}

impl Host {
    pub(crate) fn new(
        paths: Paths, env: Environment, protocols: Vec<Arc<dyn Protocol>>, local_fs: Arc<dyn FileSystem>,
    ) -> Self {
        Self { paths, env, protocols, local_fs, clock: chrono::Utc::now, local_offset: current_local_offset }
    }
}

fn current_local_offset() -> chrono::FixedOffset {
    *chrono::Local::now().offset()
}
