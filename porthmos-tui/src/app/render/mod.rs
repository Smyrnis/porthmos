use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use super::{
    hints::{form_hints, help_sections, hints_for, search_hints},
    *,
};
use crate::widgets::{
    connection_details, connection_form,
    frame::{Hint, accent_underline, fill_background, hairline, hint_bar, top_bar, vertical_rule},
    history_view::printable,
    info_line::render_info_line,
    palette_view::render_palette,
    panel_view::PanelContext,
    popup::render_popup,
    preview_view,
    session_tabs::{TabView, tabs_line},
    text_fit::truncate_with_ellipsis,
    toast::render_toast,
    transfer_line::{line_data, paused_summary, render_transfer_line},
    update_badge,
};

const DETAILS_POPUP_WIDTH: u16 = 60;
const DETAILS_POPUP_HEIGHT: u16 = 16;

impl App {
    pub(super) fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        fill_background(frame, area, &self.theme);

        let Some(areas) = layout::split_frame(area) else {
            self.render_too_small(frame, area);
            if let Some(preview) = &self.preview {
                preview_view::render_preview(frame, area, preview, &self.theme);
            }
            return;
        };
        self.body_width.set(areas.body.width);
        hairline(frame, areas.top_rule, &self.theme);
        hairline(frame, areas.bottom_rule, &self.theme);

        self.render_top_bar(frame, areas.top_bar, areas.top_rule);

        if let Some(form) = self.screen_form() {
            connection_form::render_connection_form(frame, areas.body, form, self.core.protocols(), &self.theme);
        } else {
            match self.screen {
                Screen::Files => self.render_files(frame, areas.body),
                Screen::Connections => self.render_connections(frame, areas.body),
                Screen::Search => self.render_search(frame, areas.body),
                Screen::Transfers => self.render_transfers(frame, areas.body),
                Screen::History => self.render_history(frame, areas.body),
                Screen::Sync => self.render_sync(frame, areas.body),
            }
        }

        self.render_hint_bar(frame, areas.hint_bar);

        if let Some(preview) = &self.preview {
            preview_view::render_preview(frame, area, preview, &self.theme);
        }

        if let Some(notification) = self.notifications.current() {
            let dismiss_key = (notification.severity == Severity::Error)
                .then(|| self.key_bindings.key_for(input::Scope::Global, Action::Back))
                .flatten()
                .map(input::format_key_spec);
            render_toast(
                frame,
                area,
                notification.severity,
                &notification.message,
                dismiss_key.as_deref(),
                &self.theme,
            );
        }

        if let Some(palette) = &self.palette {
            render_palette(frame, area, palette, &self.theme);
        }

        if let Some(dialog) = &self.dialog {
            dialog.render(frame, area, &self.theme);
        }

        if self.help_visible {
            let sections = help_sections(&self.key_bindings);
            self.help_max_scroll.set(help::render_help(frame, area, &sections, self.help_scroll, &self.theme));
        }

        if self.details_popup_visible() {
            self.render_details_popup(frame, area, areas.body);
        }
    }

    fn render_too_small(&self, frame: &mut Frame, area: Rect) {
        const MESSAGE: &str = "terminal too small";
        let width = area.width.min(MESSAGE.len() as u16);
        let x = area.x + (area.width - width) / 2;
        let y = area.y + area.height / 2;
        if area.height == 0 {
            return;
        }
        frame.render_widget(Paragraph::new(MESSAGE).style(self.theme.dim()), Rect::new(x, y, width, 1));
    }

    fn render_top_bar(&self, frame: &mut Frame, area: Rect, rule: Rect) {
        const NAME: &str = "porthmos";
        const GAP: u16 = 3;
        const TABS_FLOOR: u16 = 16;
        let theme = &self.theme;
        let name = Span::styled(NAME, theme.muted().add_modifier(Modifier::BOLD));
        let name_width = NAME.len() as u16;
        if let Some(label) = self.form_breadcrumb() {
            let left = Line::from(vec![
                name,
                Span::styled(" / ", theme.dim()),
                Span::styled("connections", theme.muted()),
                Span::styled(" / ", theme.dim()),
                Span::styled(label, theme.bright()),
            ]);
            let room = usize::from(area.width.saturating_sub(left.width() as u16 + GAP));
            top_bar(frame, area, left, self.top_bar_right(room, room));
            return;
        }
        let tabs = if self.screen == Screen::Files { self.tab_views() } else { Vec::new() };

        let tail = self.screen_tail();
        let tail_width = tail.as_ref().map_or(0, |tail| " / ".len() + tail.width()) as u16;
        let left_minimum = match self.screen {
            Screen::Files if tabs.is_empty() => name_width,
            Screen::Files => name_width + GAP + TABS_FLOOR.min(area.width.saturating_sub(name_width + GAP)),
            _ => name_width + " / ".len() as u16 + self.screen_title().len() as u16 + tail_width,
        };
        let left_natural = match self.screen {
            Screen::Files if !tabs.is_empty() => {
                name_width + GAP + tabs_line(&tabs, u16::MAX, theme).line.width() as u16
            }
            _ => left_minimum,
        };
        let status = self.top_bar_right(
            usize::from(area.width.saturating_sub(left_minimum + GAP)),
            usize::from(area.width.saturating_sub(left_natural + GAP)),
        );
        let status_width = status.width() as u16;

        let mut left = Line::from(name);
        if self.screen == Screen::Files {
            let reserved = name_width + GAP + if status_width > 0 { status_width + GAP } else { 0 };
            let tabs = tabs_line(&tabs, area.width.saturating_sub(reserved), theme);
            if let Some(columns) = tabs.active_columns.clone() {
                let offset = (area.x - rule.x) + name_width + GAP;
                accent_underline(frame, rule, offset + columns.start..offset + columns.end, theme);
            }
            if !tabs.line.spans.is_empty() {
                left.spans.push(Span::raw(" ".repeat(usize::from(GAP))));
                left.spans.extend(tabs.line.spans);
            }
        } else {
            left.spans.push(Span::styled(" / ", theme.dim()));
            left.spans.push(Span::styled(self.screen_title(), theme.bright()));
            if let Some(tail) = tail {
                left.spans.push(Span::styled(" / ", theme.dim()));
                left.spans.push(Span::styled(tail, theme.bright()));
            }
        }
        top_bar(frame, area, left, status);
    }

    fn top_bar_right(&self, status_room: usize, badge_room: usize) -> Line<'static> {
        let status = self.connection_status_line(status_room);
        let Some(badge) = self.update_badge() else { return status };
        let fit = badge.fit(badge_room, status.width());
        if fit.badge.is_none() {
            return status;
        }
        let status =
            if fit.status_budget < status.width() { self.connection_status_line(fit.status_budget) } else { status };
        let separated = status.width() > 0;
        let mut spans = status.spans;
        if let Some(text) = fit.badge {
            if separated {
                spans.push(Span::raw(" ".repeat(update_badge::GAP)));
            }
            spans.push(Span::styled(text, self.theme.accent_text()));
        }
        Line::from(spans)
    }

    fn screen_tail(&self) -> Option<String> {
        if self.screen != Screen::Sync {
            return None;
        }
        let name =
            self.sync_view.plan().and_then(|plan| self.sessions.by_id(plan.session)).map(|session| &session.name);
        Some(name.map_or_else(|| "remote".to_string(), |name| printable(name)))
    }

    fn screen_form(&self) -> Option<&crate::widgets::dialog::FormDialog> {
        match &self.dialog {
            Some(Dialog::Form(form)) if form.layout == crate::widgets::dialog::FormLayout::Screen => Some(form),
            _ => None,
        }
    }

    fn form_breadcrumb(&self) -> Option<&'static str> {
        self.screen_form()?;
        match self.pending_action {
            Some(PendingAction::EditConnection { .. }) => Some("edit"),
            _ => Some("new"),
        }
    }

    fn screen_title(&self) -> &'static str {
        match self.screen {
            Screen::Files => "files",
            Screen::Connections => "connections",
            Screen::Search => "search",
            Screen::Transfers => "transfers",
            Screen::History => "history",
            Screen::Sync => "sync",
        }
    }

    fn tab_views(&self) -> Vec<TabView<'_>> {
        let active_id = self.sessions.active_id();
        self.sessions
            .iter()
            .enumerate()
            .map(|(index, session)| TabView {
                number: index + 1,
                name: &session.name,
                protocol: self
                    .connections
                    .entries()
                    .iter()
                    .find(|entry| entry.name == session.name)
                    .map_or("", |entry| entry.protocol.as_str()),
                active: Some(session.id) == active_id,
                healthy: session.healthy,
            })
            .collect()
    }

    fn connection_status_line(&self, budget: usize) -> Line<'static> {
        if self.screen_form().is_some() {
            return Line::default();
        }
        if self.screen == Screen::Connections {
            return Line::styled(truncate_with_ellipsis(&self.connections_summary(), budget), self.theme.muted());
        }
        if self.screen == Screen::Sync {
            return Line::styled(truncate_with_ellipsis(&self.sync_view.top_bar_summary(), budget), self.theme.muted());
        }
        if self.screen == Screen::History {
            return Line::styled(truncate_with_ellipsis(&self.history.summary(), budget), self.theme.muted());
        }
        if self.screen == Screen::Transfers {
            let summary = transfer_list::transfer_summary(
                self.transfer_rows(),
                self.rates.aggregate_speed(),
                self.transfers.speed_limit,
            );
            return Line::styled(truncate_with_ellipsis(&summary, budget), self.theme.muted());
        }
        let (text, style) = match &self.connection_status {
            ConnectionStatus::Connecting(name) => {
                (format!("Connecting to {}\u{2026}", printable(name)), self.theme.muted())
            }
            ConnectionStatus::Failed(message) if self.sessions.is_empty() => (
                format!("Connection failed: {}", printable(message.lines().next().unwrap_or(""))),
                self.theme.err_text(),
            ),
            _ => return Line::default(),
        };
        Line::styled(truncate_with_ellipsis(&text, budget), style)
    }

    fn render_files(&self, frame: &mut Frame, area: Rect) {
        let mode = layout::files_mode(area.width);
        let line = line_data(&self.transfers.rows, &self.session_names, &self.rates);
        let paused_only = line.is_none().then(|| paused_summary(&self.transfers.rows)).flatten();
        let layout = layout::files_layout(area, mode, line.is_some() || paused_only.is_some());
        let (single, show_modified) = match mode {
            layout::FilesMode::TwoPanels => (false, true),
            layout::FilesMode::SinglePanel { show_modified } => (true, show_modified),
        };
        let now = self.clock.now();
        let theme = &self.theme;
        let local_active = self.active_panel == ActivePanel::Local;
        let remote_header = self.sessions.active().map(|session| self.remote_header(session));
        let remote_protocol = remote_header.as_ref().map_or("remote", |(protocol, _)| protocol.as_str());

        if !single || local_active {
            let context = PanelContext {
                protocol: "local",
                location: abbreviate_home(self.local.path(), self.home.as_ref()),
                active: local_active,
                show_modified,
                switch_hint: single.then(|| format!("local \u{21c4} {remote_protocol}")),
                now: &now,
            };
            panel_view::render_panel(frame, layout.local, &self.local, &context, theme);
        }

        if !single || !local_active {
            let switch_hint = single.then(|| format!("{remote_protocol} \u{21c4} local"));
            match (self.sessions.active(), &remote_header) {
                (Some(session), Some((protocol, location))) => {
                    let context = PanelContext {
                        protocol,
                        location: location.clone(),
                        active: !local_active,
                        show_modified,
                        switch_hint,
                        now: &now,
                    };
                    panel_view::render_panel(frame, layout.remote, &session.panel, &context, theme);
                }
                _ => {
                    let context = PanelContext {
                        protocol: "remote",
                        location: String::new(),
                        active: !local_active,
                        show_modified,
                        switch_hint,
                        now: &now,
                    };
                    let lines = self.remote_placeholder_lines();
                    panel_view::render_placeholder(frame, layout.remote, &lines, &context, theme);
                }
            }
        }

        if let Some(divider) = layout.divider {
            vertical_rule(frame, divider, theme);
        }
        hairline(frame, layout.rule, theme);

        let focused = match (local_active, self.sessions.active()) {
            (true, _) => self.local.current_entry(),
            (false, Some(session)) => session.panel.current_entry(),
            (false, None) => None,
        };
        render_info_line(frame, layout.info, focused, &now, theme);
        if let Some(data) = &line {
            render_transfer_line(frame, layout.transfer, data, theme);
        } else if let Some(text) = &paused_only {
            let text = truncate_with_ellipsis(text, usize::from(layout.transfer.width));
            frame.render_widget(Paragraph::new(Span::styled(text, theme.muted())), layout.transfer);
        }
    }

    fn remote_header(&self, session: &crate::sessions::Session) -> (String, String) {
        let path = session.panel.path();
        let saved = self.connections.entries().iter().find(|entry| entry.name == session.name);
        match saved {
            Some(entry) if entry.username.is_empty() => (entry.protocol.clone(), format!("{}:{path}", entry.host)),
            Some(entry) => (entry.protocol.clone(), format!("{}@{}:{path}", entry.username, entry.host)),
            None => ("remote".to_string(), format!("{}:{path}", session.name)),
        }
    }

    fn remote_placeholder_lines(&self) -> Vec<String> {
        if let ConnectionStatus::Connecting(name) = &self.connection_status {
            return vec![format!("connecting to {name}\u{2026}")];
        }
        let mut lines = vec!["not connected".to_string()];
        if let Some(spec) = self.key_bindings.key_for(input::Scope::Files, Action::OpenConnections) {
            lines.push(format!("press {} to connect", input::format_key_spec(spec)));
        }
        lines
    }

    fn render_connections(&self, frame: &mut Frame, area: Rect) {
        let name_set = |healthy: bool| -> std::collections::HashSet<&str> {
            self.sessions
                .iter()
                .filter(|session| session.healthy == healthy)
                .map(|session| session.name.as_str())
                .collect()
        };
        let (connected_names, lost_names) = (name_set(true), name_set(false));
        let context = connections_list::ConnectionsContext {
            connected: &connected_names,
            lost: &lost_names,
            failures: &self.last_failure,
            protocols: self.core.protocols(),
            last_used: &self.last_used,
            now: self.clock.now().with_timezone(&chrono::Utc),
        };
        let layout = layout::connections_layout(area);
        connections_list::render_connections_list(frame, layout.list, &self.connections, &context, &self.theme);
        if let Some(divider) = layout.divider {
            vertical_rule(frame, divider, &self.theme);
        }
        if let Some(details) = layout.details {
            self.render_details(frame, details);
        }
    }

    fn render_details(&self, frame: &mut Frame, area: Rect) {
        let selection = self.connections.selected();
        let group_count = match self.connections.rows().get(self.connections.cursor) {
            Some(porthmos_core::connections_tree::TreeRow::Group { count, .. }) => *count,
            _ => 0,
        };
        let (session_health, failure) = match &selection {
            Some(crate::widgets::connections_view::Selection::Connection(entry)) => (
                self.sessions.by_name(&entry.name).map(|session| session.healthy),
                self.last_failure.get(&entry.name).map(String::as_str),
            ),
            _ => (None, None),
        };
        let last_used = match &selection {
            Some(crate::widgets::connections_view::Selection::Connection(entry)) => {
                self.last_used.get(&entry.name).copied()
            }
            _ => None,
        };
        let context = connection_details::DetailsContext {
            protocols: self.core.protocols(),
            connected: session_health == Some(true),
            lost: session_health == Some(false),
            failure,
            last_used,
            now: self.clock.now().with_timezone(&chrono::Utc),
        };
        connection_details::render_connection_details(frame, area, selection, group_count, &context, &self.theme);
    }

    fn render_details_popup(&self, frame: &mut Frame, screen: Rect, body: Rect) {
        let size = (body.width.min(DETAILS_POPUP_WIDTH), body.height.min(DETAILS_POPUP_HEIGHT));
        let inner = render_popup(frame, screen, size, None, &self.theme);
        self.render_details(frame, inner);
    }

    fn connections_summary(&self) -> String {
        let connected = self.sessions.iter().filter(|session| session.healthy).count();
        let lost = self.sessions.iter().count() - connected;
        let errors = self.last_failure.len();
        let mut parts = Vec::new();
        if let ConnectionStatus::Connecting(name) = &self.connection_status {
            parts.push(format!("connecting to {}\u{2026}", printable(name)));
        }
        if connected > 0 {
            parts.push(format!("{connected} connected"));
        }
        if lost > 0 {
            parts.push(format!("{lost} lost"));
        }
        if errors > 0 {
            parts.push(format!("{errors} {}", if errors == 1 { "error" } else { "errors" }));
        }
        parts.join(" \u{b7} ")
    }

    fn render_transfers(&self, frame: &mut Frame, area: Rect) {
        let copy_key = self.key_bindings.key_for(input::Scope::Files, Action::Copy).map(input::format_key_spec);
        let rows = self.transfer_rows();
        let routes: Vec<String> = rows.iter().map(|row| self.route_of(row)).collect();
        let speeds: Vec<Option<u64>> = rows.iter().map(|row| self.rates.speed(row.kind)).collect();
        let etas: Vec<Option<u64>> = rows.iter().map(|row| self.rates.eta_seconds(row)).collect();
        let cursor = self.transfers_cursor.min(rows.len().saturating_sub(1));
        let failed_time =
            rows.get(cursor).and_then(|row| self.failed_time_of(row.kind)).map(|time| time.format("%H:%M").to_string());
        let view = transfer_list::TransferView {
            rows,
            cursor,
            copy_key: copy_key.as_deref(),
            routes: &routes,
            speeds: &speeds,
            etas: &etas,
            failed_time: failed_time.as_deref(),
        };
        transfer_list::render_transfer_list(frame, area, &view, &self.theme);
    }

    fn render_history(&self, frame: &mut Frame, area: Rect) {
        history_view::render_history(frame, area, &self.history, &self.theme);
    }

    fn render_sync(&self, frame: &mut Frame, area: Rect) {
        sync_view::render_sync(frame, area, &self.sync_view, &self.theme);
    }

    fn render_search(&self, frame: &mut Frame, area: Rect) {
        if let Some(session) = &self.search {
            search_view::render_search(frame, area, &session.view, &self.theme);
        }
    }

    pub(super) fn render_hint_bar(&self, frame: &mut Frame, area: Rect) {
        if let Some(text) = self.transient_status() {
            frame.render_widget(Paragraph::new(printable(&text)).style(self.theme.muted()), area);
        } else if self.screen_form().is_some() {
            hint_bar(frame, area, &form_hints(), &self.theme);
        } else if self.screen == Screen::Search {
            hint_bar(frame, area, &search_hints(), &self.theme);
        } else {
            let mut hints = hints_for(self.screen, &self.key_bindings);
            if self.screen == Screen::Connections && !self.details_pane_visible() {
                let details = self.key_bindings.key_for(input::Scope::Connections, Action::ToggleSelect);
                if let Some(spec) = details {
                    let position = 1.min(hints.entries.len());
                    hints.entries.insert(position, Hint { key: input::format_key_spec(spec), label: "details" });
                }
            }
            hint_bar(frame, area, &hints, &self.theme);
        }
    }

    fn transient_status(&self) -> Option<String> {
        self.keyring_waiting_text().or_else(|| self.waiting_for_answer_text()).or_else(|| self.planning_status_text())
    }

    fn keyring_waiting_text(&self) -> Option<String> {
        self.keyring_waiting.then(|| "Waiting for the system keyring\u{2026}".to_string())
    }

    fn waiting_for_answer_text(&self) -> Option<String> {
        if self.dialog.is_some() {
            return None;
        }
        match self.conflict_prompts.len() {
            0 => None,
            1 => Some("A copy is waiting for your answer".to_string()),
            waiting => Some(format!("{waiting} copies are waiting for your answer")),
        }
    }

    fn planning_status_text(&self) -> Option<String> {
        match self.transfers.scanning.as_slice() {
            [] => None,
            [name] => Some(format!("Scanning {name}\u{2026}")),
            scans => Some(format!("Scanning {} copies\u{2026}", scans.len())),
        }
    }
}

fn abbreviate_home(path: &VfsPath, home: Option<&VfsPath>) -> String {
    let usable_home = home.filter(|home| !home.is_empty() && !home.is_root());
    match usable_home.and_then(|home| path.strip_prefix(home)) {
        Some(rest) if rest.is_empty() => "~".to_string(),
        Some(rest) => format!("~/{rest}"),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests;
