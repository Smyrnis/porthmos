use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::widget::{Id, operation};
use iced::{Element, Point, Rectangle, Size, Subscription, Task, event, keyboard, mouse, window};
use porthmos_core::listing::{Row, SortKey};
use porthmos_core::transfer::conflicts::Resolution;
use porthmos_core::transfer::rows::RowKind;
use porthmos_core::{
    Answer, Command, CoreHandle, Event, Location, SessionId, Severity, VfsPath, from_os_path, to_os_path,
};

use porthmos_core::config::bookmarks::Bookmark;
use porthmos_core::sync::SyncOptions;

pub use self::sync::SyncChoice;
use crate::conflicts::ConflictQueue;
use crate::connections::{Manager, MissingChoice, Screen};
use crate::dialogs::{Dialog, Typed};
use crate::drag::{
    Drag, DragSource, Phase, Refusal, Release, Target, decide, decide_drop_in, dragged_entries, drop_in_refusal,
    refusal,
};
use crate::entries::entries_from_paths;
use crate::geometry::{Hit, ROW_HEIGHT, hit_test, list_viewport, scroll_to_show};
use crate::menus::{ContextMenu, MenuItem, MenuTarget, MoreItem};
use crate::modal::{Modal, ModalQueue};
use crate::names::name_problem;
use crate::native::{self, Attached, DesktopEvent, DesktopSource};
use crate::navigation::PathEdit;
use crate::pane::Pane;
use crate::panel::{Modifiers, Panel};
use crate::processes::{Launcher, ProcessEvent, ProcessSource};
use crate::runtime::{self, EventSource};
use crate::sessions::Sessions;
use crate::settings_view::GuiSettings;
use crate::shortcuts::{self, Action, KeyPress, Shortcuts};
use crate::staging::{Job, Stage, Staging};
use crate::sync_view::SyncView;
use crate::theme::Palette;
use crate::tools::Tool;
use crate::transfers::{QUEUE_HEIGHT, QueuePane, RowButton, TransferState, transfer_command};
use crate::update_view::UpdateView;
use crate::view::toolbar::ToolbarState;

const NOTICE_LIMIT: usize = 50;
const DROP_BATCH_WINDOW: Duration = Duration::from_millis(80);
const NATIVE_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STAGING_TICK: Duration = Duration::from_secs(1);
const NOTICE_TICK: Duration = Duration::from_secs(1);
const NOTICE_INFO_LIFETIME: Duration = Duration::from_secs(4);
const NOTICE_WARNING_LIFETIME: Duration = Duration::from_secs(8);
const NOTICE_ERROR_LIFETIME: Duration = Duration::from_secs(15);
const STAGING_LINGER: Duration = if cfg!(target_os = "linux") { Duration::ZERO } else { Duration::from_secs(10 * 60) };

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub severity: Severity,
    pub message: String,
    raised: Instant,
}

impl Notice {
    fn lifetime(&self) -> Duration {
        match self.severity {
            Severity::Info => NOTICE_INFO_LIFETIME,
            Severity::Warning => NOTICE_WARNING_LIFETIME,
            Severity::Error => NOTICE_ERROR_LIFETIME,
        }
    }

    fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.raised) >= self.lifetime()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSide {
    Local,
    Remote,
}

impl PanelSide {
    pub fn other(self) -> PanelSide {
        match self {
            PanelSide::Local => PanelSide::Remote,
            PanelSide::Remote => PanelSide::Local,
        }
    }

    pub fn path_input_id(self) -> Id {
        match self {
            PanelSide::Local => Id::new("local-path-input"),
            PanelSide::Remote => Id::new("remote-path-input"),
        }
    }

    pub fn list_id(self) -> Id {
        match self {
            PanelSide::Local => Id::new("local-list"),
            PanelSide::Remote => Id::new("remote-list"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Core(Event),
    CloseRequested,
    ShutdownFinished,
    ModifiersChanged(keyboard::Modifiers),
    PanelClicked(PanelSide, usize),
    ScrollChanged(PanelSide, f32),
    HeaderClicked(PanelSide, SortKey),
    TabClicked(usize),
    TransferArrowPressed(PanelSide),
    TabClosed(usize),
    PanelDoubleClicked(PanelSide, usize),
    KeyPressed(KeyPress),
    WindowResized(Size),
    ManagerToggled,
    ManagerClosed,
    ManagerFilterTyped(String),
    ManagerRowPressed(usize),
    ManagerRowActivated(usize),
    ManagerGroupToggled(String),
    ManagerConnect,
    ManagerIgnored,
    ManagerAddPressed,
    ManagerEditPressed,
    ManagerDeletePressed,
    FormTextTyped(String, String),
    FormSecretTyped(String, Typed),
    FormSecretForgotten(String),
    FormSecretRestored(String),
    FormChoiceChanged(String, String),
    FormToggled(String, bool),
    FormSavePressed,
    ScreenCancelled,
    DeleteConfirmed,
    LabelsGroupTyped(String),
    LabelsTagsTyped(String),
    LabelsForgetToggled(bool),
    LabelsSavePressed,
    MissingHostChosen(MissingChoice),
    PasswordSaveToggled(bool),
    FormTestPressed,
    FormTestCancelled,
    TestPasswordTyped(Typed),
    TestPasswordSubmitted,
    TestPasswordCancelled,
    ToolClosed,
    ToolIgnored,
    PreviewScrolled(f32),
    ProcessFinished(ProcessEvent),
    EditAnswered(porthmos_core::edit::EditChoice),
    EditDoneEditing,
    SearchTyped(String),
    SearchRowPressed(usize),
    SearchRowActivated(usize),
    SearchOpenPressed,
    BookmarkRowPressed(usize),
    BookmarkRowActivated(usize),
    BookmarkGoPressed,
    BookmarkRemovePressed,
    HistoryFilterTyped(String),
    HistoryRowPressed(usize),
    HistoryRowActivated(usize),
    HistoryDetailsPressed,
    HistoryClearPressed,
    PaletteTyped(String),
    PaletteRowPressed(usize),
    PaletteRowActivated(usize),
    UpdateBadgePressed,
    SettingsChangePressed(Action),
    SettingsResetPressed(Action),
    SettingsEditorTyped(String),
    SettingsEditorApplied,
    SettingsAutoUploadToggled(bool),
    SettingsUpdateModeChosen(porthmos_core::UpdateMode),
    SettingsScrolled(f32),
    MoreMenuToggled,
    RowRightPressed(PanelSide, usize),
    BackgroundRightPressed(PanelSide),
    MenuItemChosen(MenuItem),
    ContextMenuDismissed,
    PointerMoved(Point),
    NavigateBack(PanelSide),
    NavigateForward(PanelSide),
    NavigateUp(PanelSide),
    CrumbPressed(PanelSide, VfsPath),
    PathEditStarted(PanelSide),
    PathTyped(String),
    PathSubmitted,
    MoreItemChosen(MoreItem),
    DialogTyped(Typed),
    ModalTyped(String),
    ModalAccepted,
    ModalDeclined,
    SyncOptionChosen(SyncChoice),
    SyncFilterTyped(String),
    SyncRowPressed(usize),
    SyncTickAllPressed,
    SyncUntickAllPressed,
    SyncFlipPressed,
    SyncRunPressed,
    SyncScrolled(f32),
    ActionRequested(Action),
    QueueRowPressed(RowKind, RowButton),
    QueueClearFinished,
    QueueCancelAll,
    QueueScrolled(f32),
    SpeedLimitTyped(String),
    SpeedLimitSubmitted,
    ConflictAnswered(Option<Resolution>),
    ConflictApplyToRestToggled,
    RowPressed(PanelSide, usize),
    CursorMoved(Point),
    MouseReleased,
    Desktop(DesktopEvent),
    DesktopDropFlush,
    NativeAttached(Option<Arc<Attached>>),
    NativePoll,
    WindowLeft,
    StagingTick(Instant),
    NoticeTick(Instant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragHint {
    pub glyph: &'static str,
    pub text: String,
    pub refused: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Shown {
    local: VfsPath,
    session: Option<SessionId>,
    remote: Option<VfsPath>,
}

pub struct App {
    core: CoreHandle,
    events: EventSource,
    palette: Palette,
    notices: Vec<Notice>,
    closing: bool,
    local: Pane,
    sessions: Sessions,
    connecting: Option<String>,
    active: PanelSide,
    modifiers: keyboard::Modifiers,
    shortcuts: Shortcuts,
    filtering: Option<PanelSide>,
    filter_text: String,
    window_size: Size,
    manager: Manager,
    reveal: Option<String>,
    keyring_available: bool,
    keyring_waiting: bool,
    save_choice: bool,
    next_test_id: u64,
    running_test: Option<u64>,
    prompt_version: u64,
    announced_orphans: std::collections::HashSet<String>,
    connections_screen: Option<Screen>,
    bookmarks: Vec<Bookmark>,
    tool: Option<Tool>,
    tool_scroll: f32,
    sync_view: SyncView,
    tool_version: u64,
    next_preview_id: u64,
    launcher: Launcher,
    processes: ProcessSource,
    jobs: std::collections::HashMap<u64, editing::JobRole>,
    next_process_token: u64,
    edits_busy: bool,
    quit_confirmed: bool,
    more_open: bool,
    context_menu: Option<ContextMenu>,
    path_edit: Option<PathEdit>,
    local_home: VfsPath,
    modals: ModalQueue,
    transfers: TransferState,
    queue: QueuePane,
    conflicts: ConflictQueue,
    drag: Drag,
    pending_click: Option<(PanelSide, usize)>,
    desktop: DesktopSource,
    desktop_sender: tokio::sync::mpsc::UnboundedSender<DesktopEvent>,
    attached: Option<Arc<Attached>>,
    pointer: Option<Point>,
    desktop_hover: Option<usize>,
    drop_buffer: Vec<PathBuf>,
    drop_at: Option<Point>,
    staging: Option<Job>,
    staging_parent: PathBuf,
    update: UpdateView,
    gui_settings: GuiSettings,
    config_file: Option<PathBuf>,
}

impl App {
    pub fn new(
        core: CoreHandle, events: EventSource, local_start: VfsPath, palette: Palette, shortcuts: Shortcuts,
    ) -> App {
        let (desktop_sender, desktop) = native::channel();
        let (process_sender, processes) = crate::processes::channel();
        let mut app = App {
            core,
            events,
            palette,
            notices: Vec::new(),
            closing: false,
            local: Pane::new(local_start.clone()),
            sessions: Sessions::default(),
            connecting: None,
            active: PanelSide::Local,
            modifiers: keyboard::Modifiers::empty(),
            shortcuts,
            filtering: None,
            filter_text: String::new(),
            window_size: Size::new(1100.0, 700.0),
            manager: Manager::default(),
            reveal: None,
            keyring_available: true,
            keyring_waiting: false,
            save_choice: false,
            next_test_id: 1,
            running_test: None,
            prompt_version: 0,
            announced_orphans: std::collections::HashSet::new(),
            connections_screen: None,
            bookmarks: Vec::new(),
            tool: None,
            tool_scroll: 0.0,
            sync_view: SyncView::default(),
            tool_version: 0,
            next_preview_id: 1,
            launcher: Launcher::system(process_sender, std::env::temp_dir()),
            processes,
            jobs: std::collections::HashMap::new(),
            next_process_token: 1,
            edits_busy: false,
            quit_confirmed: false,
            more_open: false,
            context_menu: None,
            path_edit: None,
            local_home: local_start.clone(),
            modals: ModalQueue::default(),
            transfers: TransferState::default(),
            queue: QueuePane::default(),
            conflicts: ConflictQueue::default(),
            drag: Drag::default(),
            pending_click: None,
            desktop,
            desktop_sender,
            attached: None,
            pointer: None,
            desktop_hover: None,
            drop_buffer: Vec::new(),
            drop_at: None,
            staging: None,
            staging_parent: std::env::temp_dir(),
            update: UpdateView::new(porthmos_core::UpdateMode::Auto),
            gui_settings: GuiSettings::default(),
            config_file: None,
        };
        app.list(Location::Local, Some(local_start));
        app
    }

    pub fn list(&mut self, location: Location, path: Option<VfsPath>) {
        let Some(pane) = self.pane_mut(location) else {
            return;
        };
        pane.expect(path.clone());
        self.core.send(Command::List { location, path });
    }

    pub fn local(&self) -> &Pane {
        &self.local
    }

    pub fn sessions(&self) -> &Sessions {
        &self.sessions
    }

    pub fn right_panel(&self) -> Option<&Panel> {
        self.sessions.active().map(|session| &session.pane.panel)
    }

    pub fn scroll_offset(&self, side: PanelSide) -> f32 {
        match side {
            PanelSide::Local => self.local.scroll,
            PanelSide::Remote => self.sessions.active().map_or(0.0, |session| session.pane.scroll),
        }
    }

    pub fn active_side(&self) -> PanelSide {
        self.active
    }

    pub fn dialog_input_id() -> Id {
        Id::new("dialog-input")
    }

    pub fn context_menu(&self) -> Option<&ContextMenu> {
        self.context_menu.as_ref()
    }

    pub fn has_other_panel(&self, side: PanelSide) -> bool {
        self.panel_of(side.other()).is_some()
    }

    pub fn path_edit(&self) -> Option<&PathEdit> {
        self.path_edit.as_ref()
    }

    fn pane_ref(&self, side: PanelSide) -> Option<&Pane> {
        match side {
            PanelSide::Local => Some(&self.local),
            PanelSide::Remote => self.sessions.active().map(|session| &session.pane),
        }
    }

    pub fn can_go_back(&self, side: PanelSide) -> bool {
        self.pane_ref(side).is_some_and(|pane| pane.history.can_back())
    }

    pub fn can_go_forward(&self, side: PanelSide) -> bool {
        self.pane_ref(side).is_some_and(|pane| pane.history.can_forward())
    }

    pub fn can_go_up(&self, side: PanelSide) -> bool {
        self.panel_of(side).is_some_and(|panel| panel.path().parent().is_some())
    }

    pub fn more_open(&self) -> bool {
        self.more_open
    }

    pub fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    pub fn hidden_shown(&self) -> bool {
        self.panel_of(self.active).is_some_and(Panel::show_hidden)
    }

    pub fn transfer_possible(&self, from: PanelSide) -> bool {
        let to = match from {
            PanelSide::Local => PanelSide::Remote,
            PanelSide::Remote => PanelSide::Local,
        };
        self.panel_of(to).is_some() && self.panel_of(from).is_some_and(|panel| !panel.target_entries().is_empty())
    }

    pub fn running_transfer_count(&self) -> usize {
        self.transfers.rows().iter().filter(|row| !row.is_finished()).count()
    }

    pub fn toolbar_state(&self) -> ToolbarState {
        let side = self.active;
        let panel = self.panel_of(side);
        let transfer = self.transfer_possible(side);
        ToolbarState {
            copy: transfer,
            move_: transfer,
            new_folder: panel.is_some(),
            rename: panel.is_some_and(|panel| panel.cursor_entry().is_some()),
            delete: panel.is_some_and(|panel| !panel.target_entries().is_empty()),
            refresh: self.location_of(side).is_some(),
            filter: true,
            transfers: true,
        }
    }

    pub fn modal(&self) -> Option<&Modal> {
        self.modals.front()
    }

    #[cfg(test)]
    pub fn dialog(&self) -> Option<&Dialog> {
        match self.modals.front() {
            Some(Modal::Question(dialog)) => Some(dialog),
            _ => None,
        }
    }

    pub fn transfers(&self) -> &TransferState {
        &self.transfers
    }

    pub fn queue(&self) -> &QueuePane {
        &self.queue
    }

    pub fn transfer_speed_limit(&self) -> Option<u64> {
        self.transfers.speed_limit()
    }

    fn queue_extra(&self) -> f32 {
        if self.queue.is_open() { QUEUE_HEIGHT + 1.0 } else { 0.0 }
    }

    pub fn list_height(&self) -> f32 {
        list_viewport(self.window_size.height - self.queue_extra())
    }

    pub fn window_size(&self) -> Size {
        self.window_size
    }

    #[cfg(test)]
    pub fn drag(&self) -> &Drag {
        &self.drag
    }

    #[cfg(test)]
    pub fn set_staging_parent(&mut self, parent: PathBuf) {
        self.staging_parent = parent;
    }

    pub fn hint_anchor(&self) -> Option<Point> {
        if let Some(point) = self.drag.pointer() {
            return Some(point);
        }
        if self.drag.phase() != Phase::Staging {
            return None;
        }
        let point = self.pointer?;
        Some(Point::new(point.x.clamp(0.0, self.window_size.width), point.y.clamp(0.0, self.window_size.height)))
    }

    fn hit_at(&self, point: Point) -> Option<Hit> {
        let remote_scroll = self.sessions.active().map_or(0.0, |session| session.pane.scroll);
        let rows = (self.local.panel.rows().len(), self.right_panel().map_or(0, |panel| panel.rows().len()));
        hit_test(self.window_size, point, (self.local.scroll, remote_scroll), rows, self.queue_extra())
    }

    pub fn drop_target(&self) -> Option<Hit> {
        let point = self.drag.pointer().or_else(|| self.desktop_hover.and(self.pointer))?;
        self.hit_at(point)
    }

    pub fn native_task(&self) -> Task<Message> {
        let sender = self.desktop_sender.clone();
        window::oldest()
            .and_then(move |id| {
                let sender = sender.clone();
                window::run(id, move |window| native::attach(window, sender).map(Arc::new))
            })
            .map(Message::NativeAttached)
    }

    fn target_of(&self, hit: Hit) -> Option<Target> {
        let location = self.location_of(hit.side)?;
        let panel = self.panel_of(hit.side)?;
        let folder = match hit.row.and_then(|row| panel.rows().get(row)) {
            Some(Row::Entry(entry)) if entry.is_dir => entry.path.clone(),
            _ => panel.path().clone(),
        };
        Some(Target { side: hit.side, location, folder })
    }

    fn desktop_hint(&self, count: usize) -> Option<DragHint> {
        let target = self.pointer.and_then(|at| self.hit_at(at)).map(|hit| self.target_of(hit));
        if let Some(refused) = target.and_then(|target| drop_in_refusal(target.as_ref())) {
            return Some(DragHint { glyph: "✕", text: refused.message().to_string(), refused: true });
        }
        let text = match count {
            0 => "Upload files".to_string(),
            1 => "Upload 1 item".to_string(),
            count => format!("Upload {count} items"),
        };
        Some(DragHint { glyph: "+", text, refused: false })
    }

    pub fn drag_hint(&self) -> Option<DragHint> {
        if let Some(count) = self.desktop_hover {
            return self.desktop_hint(count);
        }
        if self.drag.phase() == Phase::Staging
            && let Some(job) = self.staging.as_ref().filter(|job| job.stage == Stage::Preparing)
        {
            return Some(DragHint { glyph: "↓", text: job.label(self.transfers.snapshot()), refused: false });
        }
        let (source, at) = (self.drag.source()?, self.drag.pointer()?);
        if let Some(hit) = self.hit_at(at)
            && let Some(refused) = refusal(source, self.target_of(hit).as_ref())
        {
            return Some(DragHint { glyph: "✕", text: refused.message().to_string(), refused: true });
        }
        let (glyph, verb) = if self.modifiers.shift() { ("→", "Move") } else { ("+", "Copy") };
        let count = source.entries.len();
        let noun = if count == 1 { "item" } else { "items" };
        Some(DragHint { glyph, text: format!("{verb} {count} {noun}"), refused: false })
    }

    pub fn transfer_summary(&self) -> Option<String> {
        self.transfers.summary()
    }

    pub fn filter_editing_side(&self) -> Option<PanelSide> {
        self.filtering
    }

    pub fn set_window_size(&mut self, size: Size) {
        self.window_size = size;
    }

    pub fn connecting(&self) -> Option<&str> {
        self.connecting.as_deref()
    }

    fn side_mut(&mut self, side: PanelSide) -> Option<&mut Pane> {
        match side {
            PanelSide::Local => Some(&mut self.local),
            PanelSide::Remote => self.sessions.active_mut().map(|session| &mut session.pane),
        }
    }

    fn shown(&self) -> Shown {
        Shown {
            local: self.local.panel.path().clone(),
            session: self.sessions.active().map(|session| session.id),
            remote: self.right_panel().map(|panel| panel.path().clone()),
        }
    }

    fn resync_scroll(&self, before: &Shown) -> Task<Message> {
        let after = self.shown();
        let mut tasks = Vec::new();
        if before.local != after.local {
            tasks.push(operation::scroll_to(
                PanelSide::Local.list_id(),
                operation::AbsoluteOffset { x: None, y: Some(self.local.scroll) },
            ));
        }
        if (before.session, &before.remote) != (after.session, &after.remote)
            && let Some(session) = self.sessions.active()
        {
            tasks.push(operation::scroll_to(
                PanelSide::Remote.list_id(),
                operation::AbsoluteOffset { x: None, y: Some(session.pane.scroll) },
            ));
        }
        Task::batch(tasks)
    }

    fn reveal_focused(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        for side in [PanelSide::Local, PanelSide::Remote] {
            if self.side_mut(side).is_some_and(Pane::take_reveal_request) {
                tasks.push(self.reveal_cursor(side));
            }
        }
        Task::batch(tasks)
    }

    fn pane_mut(&mut self, location: Location) -> Option<&mut Pane> {
        match location {
            Location::Local => Some(&mut self.local),
            Location::Session(id) => self.sessions.by_id_mut(id).map(|session| &mut session.pane),
        }
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }

    pub fn is_closing(&self) -> bool {
        self.closing
    }

    pub fn warn(&mut self, message: String) {
        self.push_notice(Severity::Warning, message);
    }

    fn push_notice(&mut self, severity: Severity, message: String) {
        self.notices.push(Notice { severity, message, raised: Instant::now() });
        if self.notices.len() > NOTICE_LIMIT {
            self.notices.remove(0);
        }
    }

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Notice { severity, message } => self.push_notice(severity, message),
            Event::Connecting { name } => self.connecting = Some(name),
            Event::Connected { session, name, shell_available } => {
                self.connecting = None;
                self.notices.clear();
                self.set_screen(None);
                if !self.sessions.activate(session) {
                    self.sessions.insert(session, name);
                }
                self.sessions.set_shell_available(session, shell_available);
            }
            Event::ShellReady { invocation, .. } => self.shell_ready(invocation),
            Event::ConnectFailed { message, .. } => {
                self.connecting = None;
                self.push_notice(Severity::Error, message);
            }
            Event::Disconnected { session, .. } => {
                let staging_from_it =
                    self.staging.as_ref().is_some_and(|job| job.session == session && job.stage == Stage::Preparing);
                if staging_from_it {
                    self.abort_staging();
                }
                self.close_search_on(Location::Session(session));
                self.sessions.remove(session);
                if self.sessions.is_empty() {
                    self.active = PanelSide::Local;
                    if self.filtering == Some(PanelSide::Remote) {
                        self.filtering = None;
                    }
                }
            }
            Event::SessionHealth { session, healthy } => {
                if let Some(session) = self.sessions.by_id_mut(session) {
                    session.healthy = healthy;
                }
            }
            Event::SessionCapabilities { session, preserves_times } => {
                if let Some(session) = self.sessions.by_id_mut(session) {
                    session.preserves_times = preserves_times;
                }
            }
            Event::Listed { location, path, entries } => {
                let shown = self.pane_mut(location).is_some_and(|pane| pane.accept(path, entries));
                if !shown {
                    tracing::debug!("dropped a listing for {location:?}: its folder is not the one shown");
                }
            }
            Event::FreeSpace { location, path, bytes } => {
                if let Some(pane) = self.pane_mut(location) {
                    pane.panel.set_free_space(path, bytes);
                }
            }
            Event::LocationChanged { location } => self.relist(location),
            Event::Profiles(entries) => self.profiles_arrived(entries),
            Event::ProfileSaved => self.profile_saved(),
            Event::KeyringStatus { available } => self.keyring_available = available,
            Event::KeyringWaiting { waiting } => self.keyring_waiting = waiting,
            Event::SaveChoice { save } => self.save_choice = save,
            Event::TestFinished { test_id, outcome } => self.test_finished(test_id, outcome),
            Event::TestQuestion { test_id, request_id, question } => self.test_question(test_id, request_id, question),
            Event::ProfileRejected { message } => self.profile_rejected(message),
            Event::History(entries) => self.history_arrived(entries),
            Event::Bookmarks(items) => self.bookmarks_arrived(items),
            Event::SearchFound(entry) => self.search_found(entry),
            Event::SearchDone { truncated } => self.search_done(truncated),
            Event::SearchFailed(message) => self.search_failed(message),
            Event::PreviewReady { request_id, result, .. } => self.preview_arrived(request_id, result),
            Event::EditReady { edit_id, file, editor } => self.edit_ready(edit_id, file, editor),
            Event::EditQuestion { edit_id, name, kind } => self.edit_question(edit_id, name, kind),
            Event::EditsBusy(busy) => self.edits_busy = busy,
            Event::TransfersChanged(snapshot) => {
                self.transfers.replace(snapshot);
                self.queue.snapshot_changed(self.transfers.has_unfinished());
                self.advance_staging();
            }
            Event::ConflictsFound { batch_id, files } => self.conflicts.found(batch_id, files),
            Event::ConflictsWithdrawn { batch_ids } => {
                if self.conflicts.withdraw(&batch_ids) && matches!(self.modals.front(), Some(Modal::Conflict { .. })) {
                    self.modals.pop();
                }
            }
            Event::Question { request_id, question } => {
                let save = self.keyring_available.then_some(self.save_choice);
                self.modals.push(Modal::Question(Dialog::from_question(request_id, question, save)));
            }
            Event::Update(state) => self.update_arrived(state),
            Event::SyncPlanReady(plan) => self.sync_plan_arrived(plan),
            Event::SyncWithdrawn { sync_ids } => self.sync_withdrawn(&sync_ids),
            _ => {}
        }
    }

    fn menus_allowed(&self) -> bool {
        self.modals.is_empty()
            && self.connections_screen.is_none()
            && self.tool.is_none()
            && self.filtering.is_none()
            && !self.closing
            && !self.drag.is_tracking()
    }

    fn show_menu(&mut self, side: PanelSide, target: MenuTarget) {
        let at = self.pointer.unwrap_or(Point::ORIGIN);
        self.context_menu = Some(ContextMenu { side, target, at });
    }

    fn menu_item_enabled(&self, menu: &ContextMenu, item: MenuItem) -> bool {
        crate::menus::context_items(&menu.target, self.panel_of(menu.side.other()).is_some())
            .into_iter()
            .any(|(candidate, enabled)| candidate == item && enabled)
    }

    fn open_row_menu(&mut self, side: PanelSide, row: usize) {
        if !self.menus_allowed() {
            return;
        }
        let Some(panel) = self.panel_of(side) else {
            return;
        };
        let parent_row = matches!(panel.rows().get(row), Some(Row::Parent));
        let in_selection = matches!(
            panel.rows().get(row),
            Some(Row::Entry(entry)) if panel.selected_paths().contains(&entry.path)
        );
        if in_selection {
            self.active = side;
            if let Some(pane) = self.side_mut(side) {
                pane.panel.set_cursor(row);
            }
        } else {
            let _ = self.handle(Message::PanelClicked(side, row));
        }
        let count = self.panel_of(side).map_or(0, Panel::selection_count).max(1);
        let file = !parent_row
            && count == 1
            && self.panel_of(side).and_then(Panel::cursor_entry).is_some_and(|entry| !entry.is_dir);
        let target = MenuTarget::Entries { count: if parent_row { 1 } else { count }, only_parent: parent_row, file };
        self.show_menu(side, target);
    }

    fn go_to(&mut self, side: PanelSide, path: VfsPath) {
        if let Some(location) = self.location_of(side) {
            self.list(location, Some(path));
        }
    }

    fn go_up(&mut self, side: PanelSide) {
        if let Some(parent) = self.panel_of(side).and_then(|panel| panel.path().parent()) {
            self.go_to(side, parent);
        }
    }

    fn go_history(&mut self, side: PanelSide, back: bool) {
        let Some(location) = self.location_of(side) else {
            return;
        };
        let Some(pane) = self.side_mut(side) else {
            return;
        };
        let current = pane.panel.path().clone();
        let target = if back { pane.history.back(current) } else { pane.history.forward(current) };
        if let Some(target) = target {
            pane.expect_unrecorded(Some(target.clone()));
            self.core.send(Command::List { location, path: Some(target) });
        }
    }

    fn parse_path(&self, side: PanelSide, text: &str) -> VfsPath {
        let home = (side == PanelSide::Local).then(|| text.strip_prefix('~')).flatten();
        let parsed = match (home, side) {
            (Some(rest), _) if rest.is_empty() || rest.starts_with('/') => {
                self.local_home.join(VfsPath::new(rest.trim_start_matches('/')))
            }
            (_, PanelSide::Local) => from_os_path(std::path::Path::new(text)),
            (_, PanelSide::Remote) => VfsPath::new(text),
        };
        match self.panel_of(side) {
            Some(panel) if !parsed.is_absolute() => panel.path().join(parsed),
            _ => parsed,
        }
    }

    fn relist(&mut self, location: Location) {
        let Some(pane) = self.pane_mut(location) else {
            return;
        };
        let path = Some(pane.panel.path().clone());
        self.core.send(Command::List { location, path });
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let shown_before = self.modals.version();
        let prompt_before = self.prompt_version;
        let tool_before = self.tool_version;
        let task = self.handle(message);
        let task =
            if std::mem::take(&mut self.quit_confirmed) { Task::batch([task, self.begin_close()]) } else { task };
        let task = if self.tool_version == tool_before {
            task
        } else {
            match self.tool_input_id() {
                Some(id) => Task::batch([task, operation::focus(id)]),
                None => task,
            }
        };
        self.show_next_conflict();
        let task = if self.prompt_version == prompt_before {
            task
        } else {
            Task::batch([task, operation::focus(App::test_password_id())])
        };
        if self.modals.version() == shown_before {
            return task;
        }
        let focus = match self.modals.front() {
            Some(Modal::Question(Dialog::Password { .. }) | Modal::NewFolder { .. }) => {
                operation::focus(App::dialog_input_id())
            }
            Some(Modal::Rename { .. } | Modal::BookmarkName { .. }) => {
                Task::batch([operation::focus(App::dialog_input_id()), operation::select_all(App::dialog_input_id())])
            }
            _ => return task,
        };
        Task::batch([task, focus])
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        let before = self.shown();
        match message {
            Message::Core(event) => {
                self.apply(event);
                return Task::batch([self.reveal_focused(), self.resync_scroll(&before)]);
            }
            Message::CloseRequested => return self.request_close(),
            Message::ShutdownFinished => return iced::exit(),
            Message::ModifiersChanged(modifiers) => self.modifiers = modifiers,
            Message::PanelClicked(side, row) => {
                let modifiers = Modifiers { ctrl: self.modifiers.command(), shift: self.modifiers.shift() };
                if let Some(pane) = self.side_mut(side) {
                    pane.panel.click(row, modifiers);
                    self.active = side;
                }
            }
            Message::ScrollChanged(side, offset) => {
                if let Some(pane) = self.side_mut(side) {
                    pane.scroll = offset.max(0.0);
                }
            }
            Message::HeaderClicked(side, key) => {
                if let Some(pane) = self.side_mut(side) {
                    pane.panel.sort_by(key);
                }
            }
            Message::TransferArrowPressed(from) => {
                if self.transfer_possible(from) {
                    self.copy_from(from, false);
                }
            }
            Message::TabClosed(index) => {
                if let Some(tab) = self.sessions.names().get(index) {
                    self.core.send(Command::Disconnect { session: tab.id });
                }
            }
            Message::TabClicked(index) => {
                self.sessions.set_active(index);
            }
            Message::PanelDoubleClicked(side, row) => self.open_row(side, row),
            Message::KeyPressed(press) => {
                let reveal = self.press_key(press);
                return Task::batch([reveal, self.resync_scroll(&before)]);
            }
            Message::WindowResized(size) => self.window_size = size,
            Message::MoreMenuToggled => self.more_open = !self.more_open,
            Message::PointerMoved(point) => self.pointer = Some(point),
            Message::RowRightPressed(side, row) => self.open_row_menu(side, row),
            Message::BackgroundRightPressed(side) => {
                if self.menus_allowed() && self.panel_of(side).is_some() {
                    self.active = side;
                    self.show_menu(side, MenuTarget::Background);
                }
            }
            Message::ContextMenuDismissed => self.context_menu = None,
            Message::MenuItemChosen(item) => {
                let Some(menu) = self.context_menu.take() else {
                    return Task::none();
                };
                if self.menus_allowed() && self.menu_item_enabled(&menu, item) {
                    return self.perform(item.action());
                }
            }
            Message::NavigateUp(side) => self.go_up(side),
            Message::NavigateBack(side) => self.go_history(side, true),
            Message::NavigateForward(side) => self.go_history(side, false),
            Message::CrumbPressed(side, path) => self.go_to(side, path),
            Message::PathEditStarted(side) => {
                let Some(panel) = self.panel_of(side) else {
                    return Task::none();
                };
                let text = match side {
                    PanelSide::Local => to_os_path(panel.path()).display().to_string(),
                    PanelSide::Remote => panel.path().to_string_lossy().into_owned(),
                };
                self.path_edit = Some(PathEdit { side, text });
                return operation::focus(side.path_input_id());
            }
            Message::PathTyped(text) => {
                if let Some(edit) = self.path_edit.as_mut() {
                    edit.text = text;
                }
            }
            Message::PathSubmitted => {
                if let Some(edit) = self.path_edit.take() {
                    let text = edit.text.trim();
                    if !text.is_empty() {
                        let path = self.parse_path(edit.side, text);
                        self.go_to(edit.side, path);
                    }
                }
            }
            Message::MoreItemChosen(item) => {
                self.more_open = false;
                if self.modals.is_empty() && self.filtering.is_none() && !self.closing {
                    let action = match item {
                        MoreItem::Palette => Action::Palette,
                        MoreItem::SelectAll => Action::SelectAll,
                        MoreItem::ToggleHidden => Action::ToggleHidden,
                        MoreItem::Preview => Action::Preview,
                        MoreItem::Edit => Action::Edit,
                        MoreItem::Terminal => Action::Terminal,
                        MoreItem::Search => Action::Search,
                        MoreItem::Bookmarks => Action::Bookmarks,
                        MoreItem::History => Action::History,
                        MoreItem::Sync => Action::Sync,
                        MoreItem::BookmarkHere => Action::BookmarkHere,
                        MoreItem::CheckForUpdates => Action::CheckForUpdates,
                    };
                    return self.perform(action);
                }
            }
            message @ (Message::SettingsChangePressed(_)
            | Message::SettingsResetPressed(_)
            | Message::SettingsEditorTyped(_)
            | Message::SettingsEditorApplied
            | Message::SettingsAutoUploadToggled(_)
            | Message::SettingsUpdateModeChosen(_)
            | Message::SettingsScrolled(_)) => return self.settings_message(message),
            message @ (Message::PaletteTyped(_) | Message::PaletteRowPressed(_) | Message::PaletteRowActivated(_)) => {
                return self.palette_message(message);
            }
            message @ (Message::ManagerToggled
            | Message::ManagerClosed
            | Message::ManagerFilterTyped(_)
            | Message::ManagerRowPressed(_)
            | Message::ManagerRowActivated(_)
            | Message::ManagerGroupToggled(_)
            | Message::ManagerConnect) => return self.manager_message(message),
            Message::ManagerIgnored | Message::ToolIgnored => {}
            Message::UpdateBadgePressed => self.show_update_announcement(),
            Message::ProcessFinished(event) => self.process_finished(event),
            Message::EditAnswered(choice) => self.edit_answered(choice),
            Message::EditDoneEditing => self.answer_modal(true),
            message @ (Message::ToolClosed
            | Message::SearchTyped(_)
            | Message::SearchRowPressed(_)
            | Message::SearchRowActivated(_)
            | Message::SearchOpenPressed
            | Message::BookmarkRowPressed(_)
            | Message::BookmarkRowActivated(_)
            | Message::BookmarkGoPressed
            | Message::BookmarkRemovePressed
            | Message::HistoryFilterTyped(_)
            | Message::HistoryRowPressed(_)
            | Message::HistoryRowActivated(_)
            | Message::HistoryDetailsPressed
            | Message::HistoryClearPressed
            | Message::PreviewScrolled(_)
            | Message::SyncFilterTyped(_)
            | Message::SyncRowPressed(_)
            | Message::SyncTickAllPressed
            | Message::SyncUntickAllPressed
            | Message::SyncFlipPressed
            | Message::SyncRunPressed
            | Message::SyncScrolled(_)) => return self.tool_message(message),
            Message::PasswordSaveToggled(on) => {
                if let Some(Modal::Question(Dialog::Password { save: Some(save), .. })) = self.modals.front_mut() {
                    *save = on;
                }
            }
            message @ (Message::ManagerAddPressed
            | Message::ManagerEditPressed
            | Message::ManagerDeletePressed
            | Message::FormTextTyped(..)
            | Message::FormSecretTyped(..)
            | Message::FormSecretForgotten(_)
            | Message::FormSecretRestored(_)
            | Message::FormChoiceChanged(..)
            | Message::FormToggled(..)
            | Message::FormSavePressed
            | Message::ScreenCancelled
            | Message::DeleteConfirmed
            | Message::LabelsGroupTyped(_)
            | Message::LabelsTagsTyped(_)
            | Message::LabelsForgetToggled(_)
            | Message::LabelsSavePressed
            | Message::MissingHostChosen(_)) => return self.editing_message(message),
            message @ (Message::FormTestPressed
            | Message::FormTestCancelled
            | Message::TestPasswordTyped(_)
            | Message::TestPasswordSubmitted
            | Message::TestPasswordCancelled) => return self.trial_message(message),
            Message::DialogTyped(typed) => {
                if let Some(Modal::Question(Dialog::Password { text, .. })) = self.modals.front_mut() {
                    *text = typed;
                }
            }
            Message::ModalTyped(typed) => {
                if let Some(
                    Modal::NewFolder { text, error, .. }
                    | Modal::Rename { text, error, .. }
                    | Modal::BookmarkName { text, error, .. },
                ) = self.modals.front_mut()
                {
                    *text = typed;
                    *error = None;
                }
            }
            Message::ModalAccepted => self.answer_modal(true),
            Message::ModalDeclined => self.answer_modal(false),
            Message::SyncOptionChosen(choice) => self.sync_option_chosen(choice),
            Message::QueueRowPressed(kind, button) => self.core.send(button.command(kind)),
            Message::QueueClearFinished => self.core.send(Command::ClearFinished),
            Message::QueueCancelAll => self.core.send(Command::CancelAllTransfers),
            Message::QueueScrolled(offset) => self.queue.set_scroll(offset),
            Message::SpeedLimitTyped(text) => self.queue.type_speed(text),
            Message::SpeedLimitSubmitted => {
                if let Some(bytes_per_second) = self.queue.submit_speed() {
                    self.core.send(Command::SetSpeedLimit { bytes_per_second });
                }
            }
            Message::ConflictAnswered(resolution) => self.answer_conflict(resolution),
            Message::ConflictApplyToRestToggled => self.toggle_apply_to_rest(),
            Message::RowPressed(side, row) => self.press_row(side, row),
            Message::CursorMoved(point) => {
                self.pointer = Some(point);
                self.drag.moved(point);
                if !Rectangle::with_size(self.window_size).contains(point) {
                    self.leave_window();
                }
            }
            Message::WindowLeft => self.leave_window(),
            Message::NativePoll => self.poll_native(),
            Message::NoticeTick(now) => self.notices.retain(|notice| !notice.expired(now)),
            Message::StagingTick(now) => {
                if let Some(job) = self.staging.as_mut() {
                    job.tick(now);
                }
            }
            Message::MouseReleased => {
                let release = self.drag.release();
                let pending = self.pending_click.take();
                match (release, pending) {
                    (Release::Click, Some((side, row))) => return self.handle(Message::PanelClicked(side, row)),
                    (Release::Drop { source, at }, _) => self.finish_drop(source, at),
                    (Release::Abandoned, _) => self.abort_staging(),
                    _ => {}
                }
            }
            Message::Desktop(event) => return self.desktop_event(event),
            Message::DesktopDropFlush => self.flush_desktop_drop(),
            Message::NativeAttached(attached) => self.attached = attached,
            Message::ActionRequested(action) => {
                if self.modals.is_empty() && self.filtering.is_none() && !self.closing {
                    return self.perform(action);
                }
            }
        }
        self.resync_scroll(&before)
    }

    fn panel_of(&self, side: PanelSide) -> Option<&Panel> {
        match side {
            PanelSide::Local => Some(&self.local.panel),
            PanelSide::Remote => self.right_panel(),
        }
    }

    fn prompt_problem(&self) -> Option<String> {
        if let Some(Modal::BookmarkName { text, .. }) = self.modals.front() {
            return text.trim().is_empty().then(|| "Enter a name".to_string());
        }
        let (side, text, original) = match self.modals.front()? {
            Modal::NewFolder { side, text, .. } => (*side, text, None),
            Modal::Rename { side, original, text, .. } => (*side, text, Some(original)),
            _ => return None,
        };
        if original.is_some_and(|original| &original.name == text) {
            return None;
        }
        if let Some(problem) = name_problem(text) {
            return Some(problem);
        }
        self.panel_of(side)
            .is_some_and(|panel| panel.has_entry_named(text))
            .then(|| format!("\"{text}\" already exists"))
    }

    fn press_row(&mut self, side: PanelSide, row: usize) {
        if self.connections_screen.is_some() || self.tool.is_some() {
            return;
        }
        let plain = !self.modifiers.command() && !self.modifiers.shift();
        let in_selection = self.panel_of(side).is_some_and(|panel| {
            matches!(panel.rows().get(row), Some(Row::Entry(entry)) if panel.selected_paths().contains(&entry.path))
        });
        if plain && in_selection {
            self.pending_click = Some((side, row));
            self.active = side;
        } else {
            self.pending_click = None;
            let _ = self.handle(Message::PanelClicked(side, row));
        }
        let entries = match (plain, self.panel_of(side)) {
            (true, Some(panel)) => dragged_entries(panel, row),
            _ => Vec::new(),
        };
        self.drag.press((!entries.is_empty()).then_some(DragSource { side, entries }));
    }

    fn leave_window(&mut self) {
        let Some(attached) = self.attached.clone() else {
            return;
        };
        let Some(source) = self.drag.leave() else {
            return;
        };
        self.pending_click = None;
        if source.side == PanelSide::Remote {
            self.begin_staging(source);
            return;
        }
        let paths = source.entries.iter().map(|entry| to_os_path(&entry.path)).collect();
        match attached.start_drag(paths) {
            Ok(()) => self.poll_native(),
            Err(error) => {
                self.drag.abandon();
                self.push_notice(Severity::Warning, format!("Could not drag out of the window: {error}"));
            }
        }
    }

    fn poll_native(&mut self) {
        let Some(attached) = self.attached.clone() else {
            return;
        };
        for event in attached.poll() {
            if self.drag.native_event(event) {
                self.finish_native_drag();
            }
        }
    }

    fn finish_native_drag(&mut self) {
        if self.staging.as_ref().is_some_and(|job| job.stage == Stage::Handed)
            && let Some(job) = self.staging.take()
        {
            match STAGING_LINGER.is_zero() {
                true => job.staging.remove(),
                false => job.staging.remove_after(STAGING_LINGER),
            }
        }
    }

    fn begin_staging(&mut self, source: DragSource) {
        let Some(Location::Session(session)) = self.location_of(source.side) else {
            self.drag.abandon();
            return;
        };
        let names = source.entries.iter().map(|entry| entry.name.clone()).collect();
        let staging = match Staging::create(&self.staging_parent) {
            Ok(staging) => staging.expecting(names),
            Err(error) => {
                self.drag.abandon();
                self.push_notice(Severity::Warning, format!("Could not prepare files to drag out: {error}"));
                return;
            }
        };
        let mut staging = staging;
        staging.remember(self.transfers.snapshot());
        self.core.send(Command::Copy {
            from: Location::Session(session),
            entries: source.entries.clone(),
            to: Location::Local,
            dest_dir: staging.destination(),
        });
        self.staging = Some(Job::new(staging, session, &source.entries));
    }

    fn advance_staging(&mut self) {
        let Some(job) = self.staging.as_mut() else {
            return;
        };
        job.observe(self.transfers.snapshot());
        match job.stage {
            Stage::Preparing => {
                if let Some(problem) = job.staging.failure(self.transfers.snapshot()) {
                    self.abort_staging();
                    self.push_notice(Severity::Warning, format!("Could not prepare files to drag out: {problem}"));
                } else if job.staging.finished(self.transfers.snapshot(), job.seen_a_row) {
                    self.hand_over_staged_files();
                }
            }
            Stage::Aborting => self.wind_down_staging(),
            Stage::Handed => {}
        }
    }

    fn abort_staging(&mut self) {
        self.drag.abandon();
        if let Some(job) = self.staging.as_mut().filter(|job| job.stage == Stage::Preparing) {
            job.stage = Stage::Aborting;
            self.wind_down_staging();
        }
    }

    fn wind_down_staging(&mut self) {
        let Some(job) = self.staging.as_mut() else {
            return;
        };
        let snapshot = self.transfers.snapshot();
        job.observe(snapshot);
        for kind in job.rows_to_cancel(snapshot) {
            self.core.send(Command::CancelRow { kind });
        }
        if job.wound_down(snapshot)
            && let Some(job) = self.staging.take()
        {
            job.staging.remove();
        }
    }

    fn hand_over_staged_files(&mut self) {
        let (Some(job), Some(attached)) = (self.staging.as_mut(), self.attached.clone()) else {
            return;
        };
        match attached.start_drag(job.staging.paths()) {
            Ok(()) => {
                job.stage = Stage::Handed;
                self.drag.staged();
                self.poll_native();
            }
            Err(error) => {
                self.drag.abandon();
                if let Some(job) = self.staging.take() {
                    job.staging.remove();
                }
                self.push_notice(Severity::Warning, format!("Could not drag out of the window: {error}"));
            }
        }
    }

    fn desktop_event(&mut self, event: DesktopEvent) -> Task<Message> {
        match event {
            DesktopEvent::Hover(paths) => self.desktop_hover = Some(self.desktop_hover.unwrap_or(0) + paths.len()),
            DesktopEvent::Moved(point) => self.pointer = Some(point),
            DesktopEvent::Left => self.desktop_hover = None,
            DesktopEvent::Dropped { paths, at } => {
                self.desktop_hover = None;
                let first = self.drop_buffer.is_empty();
                if first {
                    self.drop_at = at;
                }
                self.drop_buffer.extend(paths);
                if first {
                    return Task::perform(async { tokio::time::sleep(DROP_BATCH_WINDOW).await }, |()| {
                        Message::DesktopDropFlush
                    });
                }
            }
        }
        Task::none()
    }

    fn flush_desktop_drop(&mut self) {
        let paths = std::mem::take(&mut self.drop_buffer);
        let at = self.drop_at.take().or(self.pointer);
        if paths.is_empty() {
            return;
        }
        let (entries, problems) = entries_from_paths(&paths);
        for problem in problems {
            self.push_notice(Severity::Warning, problem);
        }
        let hit = match at {
            Some(point) => self.hit_at(point),
            None => Some(Hit { side: PanelSide::Remote, row: None }),
        };
        let Some(hit) = hit else {
            return;
        };
        let target = self.target_of(hit);
        match decide_drop_in(&entries, target.as_ref()) {
            Ok(command) => self.core.send(command),
            Err(Refusal::NothingDragged) => {}
            Err(refused) => self.push_notice(Severity::Warning, refused.message().to_string()),
        }
    }

    fn finish_drop(&mut self, source: DragSource, at: Point) {
        let Some(hit) = self.hit_at(at) else {
            return;
        };
        let Some(from) = self.location_of(source.side) else {
            return;
        };
        let target = self.target_of(hit);
        match decide(&source, from, target.as_ref(), self.modifiers.shift()) {
            Ok(command) => self.core.send(command),
            Err(refused) => self.push_notice(Severity::Warning, refused.message().to_string()),
        }
    }

    fn show_next_conflict(&mut self) {
        if self.modals.is_empty()
            && let Some(view) = self.conflicts.current()
        {
            self.modals.push(Modal::Conflict { view, apply_to_rest: false });
        }
    }

    fn toggle_apply_to_rest(&mut self) {
        if let Some(Modal::Conflict { apply_to_rest, .. }) = self.modals.front_mut() {
            *apply_to_rest = !*apply_to_rest;
        }
    }

    fn answer_conflict(&mut self, resolution: Option<Resolution>) {
        let Some(Modal::Conflict { view, apply_to_rest }) = self.modals.front() else {
            return;
        };
        if resolution == Some(Resolution::Resume) && !view.can_resume() {
            return;
        }
        let apply_to_rest = *apply_to_rest;
        self.modals.pop();
        if let Some(command) = self.conflicts.answer(resolution, apply_to_rest) {
            self.core.send(command);
        }
    }

    fn conflict_key(&mut self, press: &KeyPress) {
        use keyboard::Key;
        let resolution = match &press.key {
            Key::Named(keyboard::key::Named::Escape) => Some(None),
            Key::Character(text) if !press.modifiers.control() && !press.modifiers.command() => {
                match text.to_lowercase().as_str() {
                    "o" => Some(Some(Resolution::Overwrite)),
                    "s" => Some(Some(Resolution::Skip)),
                    "r" => Some(Some(Resolution::Rename)),
                    "u" => Some(Some(Resolution::Resume)),
                    "a" => {
                        self.toggle_apply_to_rest();
                        None
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(resolution) = resolution {
            self.answer_conflict(resolution);
        }
    }

    fn answer_modal(&mut self, accepted: bool) {
        if matches!(self.modals.front(), Some(Modal::Conflict { .. })) {
            if !accepted {
                self.answer_conflict(None);
            }
            return;
        }
        if accepted && let Some(problem) = self.prompt_problem() {
            if let Some(
                Modal::NewFolder { error, .. } | Modal::Rename { error, .. } | Modal::BookmarkName { error, .. },
            ) = self.modals.front_mut()
            {
                *error = Some(problem);
            }
            return;
        }
        if accepted && self.sync_options_rejected() {
            return;
        }
        if !accepted && matches!(self.modals.front(), Some(Modal::EditWaiting { .. })) {
            return;
        }
        let Some(modal) = self.modals.pop() else {
            return;
        };
        match (accepted, modal) {
            (accepted, Modal::Question(dialog)) => self.answer_question(accepted, dialog),
            (accepted, Modal::EditQuestion { edit_id, kind, .. }) => {
                self.resolve_edit_question(accepted, edit_id, kind)
            }
            (true, Modal::EditWaiting { edit_id, .. }) => self.finish_waiting_edit(edit_id),
            (true, Modal::ConfirmQuit { .. }) => self.quit_confirmed = true,
            (true, Modal::NewFolder { side, text, .. }) => {
                let parent = self.panel_of(side).map(|panel| panel.path().clone());
                if let (Some(location), Some(parent)) = (self.location_of(side), parent) {
                    self.core.send(Command::CreateDir { location, path: parent.join(text.as_str()) });
                }
            }
            (true, Modal::Rename { side, original, text, .. }) => {
                if let (Some(location), Some(parent)) = (self.location_of(side), original.path.parent())
                    && text != original.name
                {
                    self.core.send(Command::Rename { location, from: original.path, to: parent.join(text.as_str()) });
                }
            }
            (true, Modal::BookmarkName { location, path, text, .. }) => {
                self.core.send(Command::AddBookmark { label: text.trim().to_string(), location, path });
            }
            (true, Modal::ConfirmClearHistory) => self.core.send(Command::ClearHistory),
            (true, Modal::SyncOptions { session, direction, by, subfolders, .. }) => {
                self.start_sync(session, SyncOptions { direction, by, subfolders })
            }
            (true, Modal::ConfirmDelete { side, entries }) => {
                if let Some(location) = self.location_of(side) {
                    let paths = entries.into_iter().map(|entry| entry.path).collect();
                    self.core.send(Command::Delete { location, paths });
                }
            }
            (false, _) | (_, Modal::Conflict { .. } | Modal::Info { .. }) => {}
        }
    }

    fn answer_question(&mut self, accepted: bool, dialog: Dialog) {
        let request_id = dialog.request_id();
        let mut save = false;
        let answer = match (accepted, dialog) {
            (true, Dialog::Confirm { .. }) => Some(Answer::Confirmed),
            (true, Dialog::Password { text, save: choice, .. }) => {
                if let Some(chosen) = choice {
                    save = chosen;
                    self.remember_save_choice(chosen);
                }
                Some(Answer::Password(text.into_string()))
            }
            (false, _) => None,
        };
        self.core.send(Command::Answer { request_id, answer, save });
    }

    fn remember_save_choice(&mut self, save: bool) {
        if save != self.save_choice {
            self.save_choice = save;
            self.core.send(Command::RememberSaveChoice { save });
        }
    }

    fn location_of(&self, side: PanelSide) -> Option<Location> {
        match side {
            PanelSide::Local => Some(Location::Local),
            PanelSide::Remote => self.sessions.active().map(|session| Location::Session(session.id)),
        }
    }

    fn open_row(&mut self, side: PanelSide, row: usize) {
        let Some(location) = self.location_of(side) else {
            return;
        };
        let target = self.side_mut(side).and_then(|pane| pane.panel.open_target(row));
        if let Some(path) = target {
            self.list(location, Some(path));
        }
    }

    fn text_fields_are_shown(&self) -> bool {
        !self.modals.is_empty()
            || self.queue.speed_text().is_some()
            || self.path_edit.is_some()
            || self.connections_screen.is_some()
            || self.tool.is_some()
    }

    fn press_key(&mut self, press: KeyPress) -> Task<Message> {
        if self.closing {
            return Task::none();
        }
        if self.drag.is_tracking() {
            if press.key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                match (self.drag.phase(), &self.attached) {
                    (Phase::Native, Some(attached)) => attached.cancel(),
                    (Phase::Staging, _) => {
                        self.drag.cancel();
                        self.abort_staging();
                        self.pending_click = None;
                    }
                    _ => {
                        self.drag.cancel();
                        self.pending_click = None;
                    }
                }
            }
            return Task::none();
        }
        if matches!(self.modals.front(), Some(Modal::Conflict { .. })) {
            self.conflict_key(&press);
            return Task::none();
        }
        if press.key == keyboard::Key::Named(keyboard::key::Named::Tab) && self.text_fields_are_shown() {
            return if press.modifiers.shift() { operation::focus_previous() } else { operation::focus_next() };
        }
        if let Some(modal) = self.modals.front() {
            let enter_accepts = matches!(
                modal,
                Modal::Info { .. } | Modal::EditQuestion { .. } | Modal::EditWaiting { .. } | Modal::SyncOptions { .. }
            );
            let enter_declines = modal.is_risky_to_accept();
            match &press.key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => self.answer_modal(false),
                keyboard::Key::Named(keyboard::key::Named::Enter) if enter_accepts => self.answer_modal(true),
                keyboard::Key::Named(keyboard::key::Named::Enter) if enter_declines => self.answer_modal(false),
                keyboard::Key::Named(keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowDown) => {
                    self.cycle_sync_direction(true)
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowUp) => {
                    self.cycle_sync_direction(false)
                }
                _ => {}
            }
            return Task::none();
        }
        if self.queue.speed_text().is_some() {
            if press.key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                self.queue.cancel_speed();
            }
            return Task::none();
        }
        if self.path_edit.is_some() || self.more_open || self.context_menu.is_some() {
            if press.key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                self.path_edit = None;
                self.more_open = false;
                self.context_menu = None;
            }
            return Task::none();
        }
        if self.connections_screen.is_some() {
            self.manager_key(&press);
            return Task::none();
        }
        if self.tool.is_some() {
            return self.tool_key(&press);
        }
        if let Some(side) = self.filtering {
            self.type_filter(side, &press);
            return Task::none();
        }
        let (key, modifiers) = shortcuts::effective(&press);
        match self.shortcuts.action_for(&key, modifiers) {
            Some(action) => self.perform(action),
            None => Task::none(),
        }
    }

    fn type_filter(&mut self, side: PanelSide, press: &KeyPress) {
        use keyboard::Key;
        use keyboard::key::Named;
        match &press.key {
            Key::Named(Named::Escape) => {
                self.filter_text.clear();
                self.filtering = None;
            }
            Key::Named(Named::Enter) => {
                self.filtering = None;
                return self.apply_filter(side);
            }
            Key::Named(Named::Backspace) => {
                self.filter_text.pop();
            }
            _ => {
                let held = press.modifiers.control() || press.modifiers.alt() || press.modifiers.logo();
                match (&press.text, held) {
                    (Some(text), false) => self.filter_text.extend(text.chars().filter(|c| !c.is_control())),
                    _ => return,
                }
            }
        }
        self.apply_filter(side);
    }

    fn apply_filter(&mut self, side: PanelSide) {
        let text = self.filter_text.clone();
        if let Some(pane) = self.side_mut(side) {
            pane.panel.set_filter(Some(&text).filter(|text| !text.is_empty()).map(String::as_str));
        }
    }

    fn page_rows(&self) -> isize {
        ((self.list_height() / ROW_HEIGHT) as isize - 1).max(1)
    }

    fn reveal_cursor(&mut self, side: PanelSide) -> Task<Message> {
        let viewport = self.list_height();
        let Some(pane) = self.side_mut(side) else {
            return Task::none();
        };
        let target = scroll_to_show(pane.panel.cursor(), pane.scroll, viewport, ROW_HEIGHT);
        if target == pane.scroll {
            return Task::none();
        }
        pane.scroll = target;
        operation::scroll_to(side.list_id(), operation::AbsoluteOffset { x: None, y: Some(target) })
    }

    fn copy_active(&mut self, moving: bool) {
        self.copy_from(self.active, moving);
    }

    fn copy_from(&mut self, from_side: PanelSide, moving: bool) {
        let to_side = match from_side {
            PanelSide::Local => PanelSide::Remote,
            PanelSide::Remote => PanelSide::Local,
        };
        let Some(dest_dir) = self.panel_of(to_side).map(|panel| panel.path().clone()) else {
            self.push_notice(
                Severity::Info,
                "Connect to a server first: copy and move go between the two panels".to_string(),
            );
            return;
        };
        let entries = self.panel_of(from_side).map(Panel::target_entries).unwrap_or_default();
        if entries.is_empty() {
            return;
        }
        if let (Some(from), Some(to)) = (self.location_of(from_side), self.location_of(to_side)) {
            self.core.send(transfer_command(moving, (from, entries), (to, dest_dir)));
        }
    }

    fn perform(&mut self, action: Action) -> Task<Message> {
        let side = self.active;
        let page = self.page_rows();
        match action {
            Action::MoveUp | Action::MoveDown | Action::PageUp | Action::PageDown | Action::Home | Action::End => {
                if let Some(pane) = self.side_mut(side) {
                    match action {
                        Action::MoveUp => pane.panel.move_cursor(-1),
                        Action::MoveDown => pane.panel.move_cursor(1),
                        Action::PageUp => pane.panel.move_cursor(-page),
                        Action::PageDown => pane.panel.move_cursor(page),
                        Action::Home => pane.panel.set_cursor(0),
                        _ => pane.panel.set_cursor(usize::MAX),
                    }
                }
                return self.reveal_cursor(side);
            }
            Action::Open => {
                if let Some(pane) = self.side_mut(side) {
                    let row = pane.panel.cursor();
                    self.open_row(side, row);
                }
            }
            Action::Parent => self.go_up(side),
            Action::SwitchPanel => {
                self.active = match (self.active, self.sessions.is_empty()) {
                    (PanelSide::Local, false) => PanelSide::Remote,
                    _ => PanelSide::Local,
                };
            }
            Action::ToggleSelect => {
                if let Some(pane) = self.side_mut(side) {
                    let row = pane.panel.cursor();
                    pane.panel.click(row, Modifiers { ctrl: true, shift: false });
                }
            }
            Action::SelectAll => {
                if let Some(pane) = self.side_mut(side) {
                    pane.panel.select_all();
                }
            }
            Action::ToggleHidden => {
                if let Some(pane) = self.side_mut(side) {
                    pane.panel.toggle_hidden();
                }
            }
            Action::CycleSort => {
                if let Some(pane) = self.side_mut(side) {
                    pane.panel.cycle_sort();
                }
            }
            Action::Refresh => {
                if let Some(location) = self.location_of(side) {
                    self.relist(location);
                }
            }
            Action::StartFilter => {
                if let Some(pane) = self.side_mut(side) {
                    let current = pane.panel.filter().unwrap_or("").to_string();
                    self.filter_text = current;
                    self.filtering = Some(side);
                }
            }
            Action::ClearFilter => {
                self.filter_text.clear();
                if let Some(pane) = self.side_mut(side) {
                    pane.panel.set_filter(None);
                }
            }
            Action::NewFolder => {
                if self.panel_of(side).is_some() {
                    self.modals.push(Modal::NewFolder { side, text: String::new(), error: None });
                }
            }
            Action::Rename => {
                let original = self.panel_of(side).and_then(Panel::cursor_entry);
                if let Some(original) = original {
                    let text = original.name.clone();
                    self.modals.push(Modal::Rename { side, original, text, error: None });
                }
            }
            Action::Delete => {
                let entries = self.panel_of(side).map(Panel::target_entries).unwrap_or_default();
                if !entries.is_empty() {
                    self.modals.push(Modal::ConfirmDelete { side, entries });
                }
            }
            Action::Copy => self.copy_active(false),
            Action::Move => self.copy_active(true),
            Action::ToggleQueue => self.queue.toggle(self.transfers.has_unfinished()),
            Action::Preview => self.open_preview(),
            Action::Edit => self.start_edit(),
            Action::Terminal => self.request_terminal(),
            Action::Search => self.open_search(),
            Action::Bookmarks => self.open_bookmarks(),
            Action::History => self.open_history(),
            Action::Sync => self.request_sync(),
            Action::Palette => self.open_palette(),
            Action::Settings => self.open_settings(),
            Action::CheckForUpdates => self.check_for_updates(),
            Action::BookmarkHere => self.open_bookmark_dialog(),
            Action::NextTab => self.sessions.next(),
            Action::PreviousTab => self.sessions.previous(),
            Action::Quit => return self.handle(Message::CloseRequested),
        }
        Task::none()
    }

    fn pointer_subscription(&self) -> Subscription<Message> {
        if !self.drag.is_tracking() && self.desktop_hover.is_none() {
            return Subscription::none();
        }
        event::listen_with(|event, _, _| match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::CursorMoved(position)),
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::MouseReleased),
            iced::Event::Mouse(mouse::Event::CursorLeft) => Some(Message::WindowLeft),
            _ => None,
        })
    }

    fn pointer_position_subscription() -> Subscription<Message> {
        event::listen_with(|event, _, _| match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::PointerMoved(position)),
            _ => None,
        })
    }

    fn native_subscription(&self) -> Subscription<Message> {
        if self.drag.phase() == Phase::Native {
            iced::time::every(NATIVE_POLL_INTERVAL).map(|_| Message::NativePoll)
        } else {
            Subscription::none()
        }
    }

    fn staging_subscription(&self) -> Subscription<Message> {
        if self.drag.phase() == Phase::Staging {
            iced::time::every(STAGING_TICK).map(Message::StagingTick)
        } else {
            Subscription::none()
        }
    }

    fn notice_subscription(&self) -> Subscription<Message> {
        if self.notices.is_empty() {
            Subscription::none()
        } else {
            iced::time::every(NOTICE_TICK).map(Message::NoticeTick)
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            runtime::subscription(&self.events).map(Message::Core),
            window::close_requests().map(|_| Message::CloseRequested),
            event::listen_with(|event, status, _| match event {
                iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                    Some(Message::ModifiersChanged(modifiers))
                }
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modified_key, modifiers, text, .. })
                    if status == event::Status::Ignored
                        || key == keyboard::Key::Named(keyboard::key::Named::Escape) =>
                {
                    let text = text.map(|text| text.to_string());
                    Some(Message::KeyPressed(KeyPress { key, modified_key, modifiers, text }))
                }
                _ => None,
            }),
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
            self.pointer_subscription(),
            Self::pointer_position_subscription(),
            self.native_subscription(),
            self.staging_subscription(),
            self.notice_subscription(),
            self.desktop.subscription().map(Message::Desktop),
            self.processes.subscription().map(Message::ProcessFinished),
            event::listen_with(|event, _, _| match event {
                iced::Event::Window(window::Event::FileHovered(path)) => {
                    Some(Message::Desktop(DesktopEvent::Hover(vec![path])))
                }
                iced::Event::Window(window::Event::FileDropped(path)) => {
                    Some(Message::Desktop(DesktopEvent::Dropped { paths: vec![path], at: None }))
                }
                iced::Event::Window(window::Event::FilesHoveredLeft) => Some(Message::Desktop(DesktopEvent::Left)),
                _ => None,
            }),
        ])
    }

    pub fn view(&self) -> Element<'_, Message> {
        crate::view::root(self)
    }
}

mod connections;
mod editing;
mod palette;
mod preview;
mod quit;
mod settings;
mod sync;
mod terminal;
mod tools;
mod update;

#[cfg(test)]
mod tests;
