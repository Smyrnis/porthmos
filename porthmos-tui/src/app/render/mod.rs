use std::path::Path;

use ratatui::{
    style::Modifier,
    text::{Line, Span},
};

use super::{
    hints::{form_hints, hints_for},
    *,
};
use crate::widgets::{
    connection_details, connection_form,
    frame::{accent_underline, clear_popup, fill_background, hairline, hint_bar, top_bar, vertical_rule},
    history_view::printable,
    info_line::render_info_line,
    panel_view::PanelContext,
    session_tabs::{TabView, tabs_line},
    text_fit::truncate_with_ellipsis,
    toast::render_toast,
    transfer_line::{line_data, render_transfer_line},
};

impl App {
    pub(super) fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        fill_background(frame, area, &self.theme);

        let Some(areas) = layout::split_frame(area) else {
            self.render_too_small(frame, area);
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

        if let Some(notification) = self.notifications.current() {
            render_toast(frame, area, notification.severity, &notification.message, &self.theme);
        }

        if let Some(dialog) = &self.dialog {
            dialog.render(frame, area, &self.theme);
        }

        if self.help_visible {
            help::render_help(frame, area, &self.key_bindings, &self.theme);
        }

        if self.details_popup && self.screen == Screen::Connections {
            self.render_details_popup(frame, areas.body);
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
            top_bar(frame, area, left, Line::default());
            return;
        }
        let tabs = if self.screen == Screen::Files { self.tab_views() } else { Vec::new() };

        let left_minimum = match self.screen {
            Screen::Files if tabs.is_empty() => name_width,
            Screen::Files => name_width + GAP + TABS_FLOOR.min(area.width.saturating_sub(name_width + GAP)),
            _ => name_width + " / ".len() as u16 + self.screen_title().len() as u16,
        };
        let status = self.connection_status_line(usize::from(area.width.saturating_sub(left_minimum + GAP)));
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
        }
        top_bar(frame, area, left, status);
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
        if self.screen == Screen::Transfers {
            let summary = transfer_list::transfer_summary(self.transfer_rows(), self.rates.aggregate_speed());
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
        let layout = layout::files_layout(area, mode, line.is_some());
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
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let context = PanelContext {
                protocol: "local",
                location: abbreviate_home(self.local.path(), home.as_deref()),
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
        }
    }

    fn remote_header(&self, session: &crate::sessions::Session) -> (String, String) {
        let path = session.panel.path().display();
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
        if let Some(spec) = self.key_bindings.key_for(Action::OpenConnections) {
            lines.push(format!("press {} to connect", input::format_key_spec(spec)));
        }
        lines
    }

    fn render_connections(&self, frame: &mut Frame, area: Rect) {
        let connected_names: std::collections::HashSet<&str> =
            self.sessions.iter().map(|session| session.name.as_str()).collect();
        let context = connections_list::ConnectionsContext {
            connected: &connected_names,
            failures: &self.last_failure,
            protocols: self.core.protocols(),
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
        let (connected, failure) = match &selection {
            Some(crate::widgets::connections_view::Selection::Connection(entry)) => {
                (self.sessions.by_name(&entry.name).is_some(), self.last_failure.get(&entry.name).map(String::as_str))
            }
            _ => (false, None),
        };
        let context = connection_details::DetailsContext { protocols: self.core.protocols(), connected, failure };
        connection_details::render_connection_details(frame, area, selection, group_count, &context, &self.theme);
    }

    fn render_details_popup(&self, frame: &mut Frame, body: Rect) {
        let width = body.width.min(60);
        let height = body.height.min(16);
        let popup = Rect::new(body.x + (body.width - width) / 2, body.y + (body.height - height) / 2, width, height);
        clear_popup(frame, popup, &self.theme);
        let block = ratatui::widgets::Block::bordered().border_style(self.theme.popup_border());
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        self.render_details(frame, inner);
    }

    fn connections_summary(&self) -> String {
        let connected = self.sessions.iter().count();
        let errors = self.last_failure.len();
        let mut parts = Vec::new();
        if connected > 0 {
            parts.push(format!("{connected} connected"));
        }
        if errors > 0 {
            parts.push(format!("{errors} {}", if errors == 1 { "error" } else { "errors" }));
        }
        parts.join(" \u{b7} ")
    }

    fn render_transfers(&self, frame: &mut Frame, area: Rect) {
        let copy_key = self.key_bindings.key_for(Action::Copy).map(input::format_key_spec);
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
        let name = self
            .sync_view
            .plan()
            .and_then(|plan| self.sessions.by_id(plan.session))
            .map_or("remote", |session| session.name.as_str());
        let title = format!("Sync with {name}");
        sync_view::render_sync(frame, area, &self.sync_view, &title, &self.theme);
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
        } else {
            hint_bar(frame, area, &hints_for(self.screen, &self.key_bindings), &self.theme);
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

fn abbreviate_home(path: &Path, home: Option<&Path>) -> String {
    let usable_home = home.filter(|home| !home.as_os_str().is_empty() && *home != Path::new("/"));
    match usable_home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests;
