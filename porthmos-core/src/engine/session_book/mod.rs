use std::collections::{BTreeMap, HashMap, HashSet};

use porthmos_vfs::VfsPath;

use super::{LiveSession, SessionId};

#[derive(Default)]
pub(crate) struct SessionBook {
    pub(crate) sessions: BTreeMap<SessionId, LiveSession>,
    pub(crate) lost_sessions: HashSet<SessionId>,
    pub(crate) session_folders: HashMap<SessionId, VfsPath>,
    pub(crate) resume_folders: HashMap<String, VfsPath>,
    pub(crate) connecting: HashSet<String>,
    next_session_id: SessionId,
}

impl SessionBook {
    pub(crate) fn allocate_id(&mut self) -> SessionId {
        let id = self.next_session_id;
        self.next_session_id += 1;
        id
    }
}

#[cfg(test)]
mod tests;
