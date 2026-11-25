use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};

use porthmos_tunnel::{StatsSnapshot, TunnelKey, TunnelSpec};
use porthmos_vfs::{Entry, Question, ShellInvocation, ShellStream, VfsPath};

use super::{Location, RequestId, SessionId, TerminalId};
use crate::{
    Completion, ConnectionState, Handover, Message, Severity,
    config::bookmarks::Bookmark,
    delete_prompt::count::Contents,
    diff::DiffOutcome,
    edit::{EditQuestionKind, EditorCommand},
    history::HistoryEntry,
    import::ImportPlan,
    preview::Preview,
    profiles::ConnectionEntry,
    properties::{Digest, Measured},
    sync::SyncPlan,
    test_connection::TestOutcome,
    transfer::{TransferSnapshot, conflicts::ConflictInfo},
    update::UpdateState,
};

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelStatus {
    Stopped,
    Offline,
    Starting,
    Running { local: SocketAddr },
    Failed(Message),
    Lost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TunnelInfo {
    pub spec: TunnelSpec,
    pub status: TunnelStatus,
    pub stats: StatsSnapshot,
}

#[derive(Debug, Clone)]
pub enum Event {
    Notice { severity: Severity, message: Message },
    Connecting { name: String },
    ReopenOffer { names: Vec<String> },
    FocusConnection { name: String },
    Connected { session: SessionId, name: String, shell_available: bool },
    SessionCapabilities { session: SessionId, preserves_times: bool, embedded_shell: bool },
    ConnectFailed { name: String, message: Message },
    Question { request_id: RequestId, question: Question },
    TestQuestion { test_id: u64, request_id: RequestId, question: Question },
    TestFinished { test_id: u64, outcome: TestOutcome },
    Disconnected { session: SessionId, name: String },
    SessionHealth { session: SessionId, healthy: bool },
    ConnectionState { name: String, state: ConnectionState },
    ShellReady { session: SessionId, invocation: ShellInvocation },
    LocalShellReady { invocation: ShellInvocation },
    TerminalOpened { id: TerminalId, session: SessionId, stream: Handover<ShellStream> },
    TerminalOpenFailed { id: TerminalId, message: Message },
    Listed { location: Location, path: VfsPath, entries: Vec<Entry> },
    Revealed { location: Location, folder: VfsPath, name: String },
    PathOpened { location: Location },
    PathRejected { location: Location },
    PathCompleted { location: Location, text: String, completion: Completion },
    FreeSpace { location: Location, path: VfsPath, bytes: Option<u64> },
    LocationChanged { location: Location },
    Profiles(Vec<ConnectionEntry>),
    LastUsed(BTreeMap<String, DateTime<Utc>>),
    ProfileSaved,
    ProfileRejected { field: Option<&'static str>, message: String },
    Bookmarks(Vec<Bookmark>),
    TransfersChanged(TransferSnapshot),
    ConflictsFound { batch_id: u64, files: Vec<ConflictInfo> },
    ConflictsWithdrawn { batch_ids: Vec<u64> },
    SearchFound { request_id: RequestId, entry: Entry },
    SearchDone { request_id: RequestId, truncated: bool, unreadable: usize },
    SearchFailed { request_id: RequestId, message: Message },
    KeyringStatus { available: bool },
    KeyringWaiting { waiting: bool },
    SaveChoice { save: bool },
    History(Vec<HistoryEntry>),
    EditReady { edit_id: u64, file: PathBuf, editor: EditorCommand },
    EditQuestion { edit_id: u64, name: String, kind: EditQuestionKind },
    EditDiff { edit_id: u64, result: Result<Arc<DiffOutcome>, Message> },
    EditsBusy(bool),
    ContentsCounted { request_id: RequestId, contents: Contents },
    MeasureProgress { request_id: RequestId, measured: Measured },
    Measured { request_id: RequestId, result: Result<Measured, Message> },
    ChecksumProgress { request_id: RequestId, path: VfsPath, bytes_done: u64, bytes_total: u64 },
    Checksummed { request_id: RequestId, path: VfsPath, result: Result<Digest, Message> },
    PreviewReady { request_id: RequestId, name: String, result: Result<Arc<Preview>, Message> },
    SyncPlanReady(Arc<SyncPlan>),
    SyncWithdrawn { sync_ids: Vec<u64> },
    ImportPlanReady(Arc<ImportPlan>),
    ImportReadFailed { import_id: u64, message: Message },
    ImportDone { import_id: u64, added: usize, skipped: usize },
    Update(UpdateState),
    Tunnels(Vec<TunnelInfo>),
    TunnelStatus { key: TunnelKey, status: TunnelStatus },
    TunnelStats(Vec<(TunnelKey, StatsSnapshot)>),
}
