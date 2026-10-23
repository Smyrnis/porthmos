use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};

use porthmos_vfs::{Entry, Question, ShellInvocation};

use super::{Location, RequestId, SessionId};
use crate::{
    Severity,
    config::bookmarks::Bookmark,
    edit::{EditQuestionKind, EditorCommand},
    history::HistoryEntry,
    preview::Preview,
    profiles::ConnectionEntry,
    sync::SyncPlan,
    test_connection::TestOutcome,
    transfer::{TransferSnapshot, conflicts::ConflictInfo},
};

#[derive(Debug, Clone)]
pub enum Event {
    Notice { severity: Severity, message: String },
    Connecting { name: String },
    Connected { session: SessionId, name: String, shell_available: bool },
    SessionCapabilities { session: SessionId, preserves_times: bool },
    ConnectFailed { name: String, message: String },
    Question { request_id: RequestId, question: Question },
    TestQuestion { test_id: u64, request_id: RequestId, question: Question },
    TestFinished { test_id: u64, outcome: TestOutcome },
    Disconnected { session: SessionId, name: String },
    SessionHealth { session: SessionId, healthy: bool },
    ShellReady { session: SessionId, invocation: ShellInvocation },
    Listed { location: Location, path: PathBuf, entries: Vec<Entry> },
    FreeSpace { location: Location, path: PathBuf, bytes: Option<u64> },
    LocationChanged { location: Location },
    Profiles(Vec<ConnectionEntry>),
    LastUsed(BTreeMap<String, DateTime<Utc>>),
    ProfileSaved,
    ProfileRejected { message: String },
    Bookmarks(Vec<Bookmark>),
    TransfersChanged(TransferSnapshot),
    ConflictsFound { batch_id: u64, files: Vec<ConflictInfo> },
    ConflictsWithdrawn { batch_ids: Vec<u64> },
    SearchFound(Entry),
    SearchDone { truncated: bool },
    SearchFailed(String),
    KeyringStatus { available: bool },
    KeyringWaiting { waiting: bool },
    SaveChoice { save: bool },
    History(Vec<HistoryEntry>),
    EditReady { edit_id: u64, file: PathBuf, editor: EditorCommand },
    EditQuestion { edit_id: u64, name: String, kind: EditQuestionKind },
    EditsBusy(bool),
    PreviewReady { request_id: RequestId, name: String, result: Result<Arc<Preview>, String> },
    SyncPlanReady(Arc<SyncPlan>),
    SyncWithdrawn { sync_ids: Vec<u64> },
}
