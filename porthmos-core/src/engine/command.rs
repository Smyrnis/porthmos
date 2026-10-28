use porthmos_vfs::{Answer, Entry, VfsPath};

use crate::{
    edit::{EditChoice, EditorExit},
    profiles::ProfileDraft,
    sync::{SyncAction, SyncOptions},
    transfer::{conflicts::Resolution, rows::RowKind},
};

pub type SessionId = u64;
pub type RequestId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Location {
    Local,
    Session(SessionId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Connect { profile: String },
    Disconnect { session: SessionId },
    TestConnection { test_id: u64, original: Option<String>, draft: Box<ProfileDraft> },
    CancelTest,
    Answer { request_id: RequestId, answer: Option<Answer>, save: bool },
    PrepareShell { session: SessionId, directory: Option<VfsPath> },
    List { location: Location, path: Option<VfsPath> },
    CreateDir { location: Location, path: VfsPath },
    Rename { location: Location, from: VfsPath, to: VfsPath },
    Delete { location: Location, paths: Vec<VfsPath> },
    Copy { from: Location, entries: Vec<Entry>, to: Location, dest_dir: VfsPath },
    Move { from: Location, entries: Vec<Entry>, to: Location, dest_dir: VfsPath },
    ResolveConflicts { batch_id: u64, answers: Option<Vec<Resolution>> },
    CancelAllTransfers,
    SetSpeedLimit { bytes_per_second: Option<u64> },
    CancelRow { kind: RowKind },
    PauseRow { kind: RowKind },
    ResumeRow { kind: RowKind },
    ReorderRow { kind: RowKind, earlier: bool },
    RetryRow { kind: RowKind },
    ClearFinished,
    Search { location: Location, root: VfsPath, pattern: String },
    CancelSearch,
    ListProfiles,
    SaveProfile { original: Option<String>, draft: Box<ProfileDraft> },
    DeleteProfile { name: String },
    SaveSshLabels { name: String, group: String, tags: String },
    MoveSshLabels { from: String, to: String },
    ForgetSshLabels { name: String },
    RememberSaveChoice { save: bool },
    ForgetSshPassword { alias: String },
    AddBookmark { label: String, location: Location, path: VfsPath },
    RemoveBookmark { index: usize },
    ListHistory,
    ClearHistory,
    EditFile { location: Location, path: VfsPath },
    Preview { request_id: RequestId, location: Location, path: VfsPath },
    CancelPreview,
    FinishEdit { edit_id: u64, exit: EditorExit },
    ResolveEdit { edit_id: u64, choice: EditChoice },
    StartSync { session: SessionId, local_dir: VfsPath, remote_dir: VfsPath, options: SyncOptions },
    RunSync { sync_id: u64, choices: Vec<(u32, SyncAction)> },
    CancelSync { sync_id: u64 },
    CheckForUpdate { manual: bool },
    Shutdown,
}
