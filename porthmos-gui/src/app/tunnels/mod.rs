use std::time::{Duration, Instant};

use porthmos_core::i18n::label as tr;
use porthmos_core::{Command, Severity, TunnelInfo, TunnelKey, TunnelStats, TunnelStatus};

use super::App;
use crate::drawer::DrawerView;
use crate::modal::Modal;
use crate::tunnel_form::{DraftChange, Field, TunnelDraft};
use crate::tunnels_view::TunnelsState;
use crate::view::tunnel_list::stoppable;

const FIRST_STATS_INTERVAL: Duration = Duration::from_secs(1);

pub(in crate::app) struct TunnelFeed {
    pub state: TunnelsState,
    pub last_stats: Option<Instant>,
}

impl TunnelFeed {
    pub fn new() -> Self {
        Self { state: TunnelsState::default(), last_stats: None }
    }
}

impl App {
    pub fn tunnels(&self) -> &TunnelsState {
        &self.tunnel_feed.state
    }

    pub fn tunnels_tab_label(&self) -> String {
        self.tunnel_feed.state.tab_label()
    }

    pub(in crate::app) fn tunnels_arrived(&mut self, rows: Vec<TunnelInfo>) {
        self.tunnel_feed.state.apply_list(rows);
    }

    pub(in crate::app) fn tunnel_status_arrived(&mut self, key: &TunnelKey, status: TunnelStatus) {
        self.tunnel_feed.state.apply_status(key, status);
    }

    pub(in crate::app) fn tunnel_stats_arrived(&mut self, batch: &[(TunnelKey, TunnelStats)]) {
        let now = Instant::now();
        let elapsed = self.tunnel_feed.last_stats.map_or(FIRST_STATS_INTERVAL, |before| now.duration_since(before));
        self.tunnel_feed.last_stats = Some(now);
        self.tunnel_feed.state.apply_stats(batch, elapsed);
    }

    pub(in crate::app) fn show_tunnels(&mut self) {
        self.dismiss_inline_editors();
        self.drawer.view = DrawerView::Tunnels;
        self.drawer.open = true;
        self.drawer.focused = false;
    }

    pub(in crate::app) fn tunnel_command(&self, command: Command) {
        self.core.send(command);
    }

    pub(in crate::app) fn tunnel_form_requested(&mut self, original: Option<TunnelKey>) {
        let (draft, connections) = match original {
            Some(key) => {
                let Some(row) = self.tunnel_feed.state.rows.iter().find(|row| row.spec.key() == key) else {
                    return;
                };
                (TunnelDraft::from_spec(&row.spec), vec![row.spec.connection.clone()])
            }
            None => {
                let connections: Vec<String> = self.sessions.names().into_iter().map(|tab| tab.name).collect();
                let active = self.sessions.active().map(|session| session.name.clone());
                let Some(first) = active.or_else(|| connections.first().cloned()) else {
                    return self.push_notice(Severity::Warning, tr("Connect to a server first"));
                };
                (TunnelDraft::new(&first), connections)
            }
        };
        self.show_tunnels();
        self.modals.push(Modal::Tunnel { draft: Box::new(draft), connections, errors: Vec::new() });
    }

    pub(in crate::app) fn tunnel_draft_changed(&mut self, change: DraftChange) {
        let Some(Modal::Tunnel { draft, errors, .. }) = self.modals.front_mut() else {
            return;
        };
        let cleared: &[Field] = match &change {
            DraftChange::Connection(_) => &[Field::Connection],
            DraftChange::Name(_) => &[Field::Name],
            DraftChange::Kind(_) => &[Field::TargetHost, Field::TargetPort],
            DraftChange::ListenHost(_) | DraftChange::AllowExternal(_) => &[Field::ListenHost],
            DraftChange::ListenPort(_) => &[Field::ListenPort],
            DraftChange::TargetHost(_) => &[Field::TargetHost],
            DraftChange::TargetPort(_) => &[Field::TargetPort],
            DraftChange::AutoStart(_) => &[],
        };
        errors.retain(|(field, _)| !cleared.contains(field));
        draft.apply(change);
    }

    pub(in crate::app) fn tunnel_form_rejected(&mut self) -> bool {
        let Some(Modal::Tunnel { draft, errors, .. }) = self.modals.front_mut() else {
            return false;
        };
        match draft.build() {
            Ok(_) => false,
            Err(found) => {
                *errors = found;
                true
            }
        }
    }

    pub(in crate::app) fn save_tunnel_draft(&mut self, draft: &TunnelDraft) {
        let Ok(spec) = draft.build() else {
            return;
        };
        let was_running = draft.original.as_ref().is_some_and(|original| {
            self.tunnel_feed.state.rows.iter().any(|row| row.spec.key() == *original && stoppable(&row.status))
        });
        let key = spec.key();
        self.core.send(Command::SaveTunnel { original: draft.original.clone(), spec: Box::new(spec) });
        if was_running {
            self.modals.push(Modal::ConfirmRestartTunnel { key });
        }
    }
}

#[cfg(test)]
mod tests;
