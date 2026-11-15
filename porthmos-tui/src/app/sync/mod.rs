use porthmos_core::i18n;
use std::sync::Arc;

use porthmos_core::{
    delete_prompt::this_computer,
    sync::{SyncAction, SyncBy, SyncDirection, SyncOptions, SyncPlan, mirror_delete_prompt},
};

use super::*;
use crate::widgets::{
    dialog::{FormDialog, FormField},
    filter_line::FilterLine,
    paging::Paged,
};

const DIRECTION: &str = "direction";
const BY: &str = "by";
const SUBFOLDERS: &str = "subfolders";
const MIRROR: &str = "mirror";

fn choices(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(value, key)| (value.to_string(), i18n::text(key))).collect()
}

impl App {
    pub(super) fn open_sync(&mut self) {
        if self.screen != Screen::Files {
            return;
        }
        if self.sync_view.has_plan() {
            self.screen = Screen::Sync;
            return;
        }
        let Some(session) = self.sessions.active() else {
            self.notifications.push(Severity::Warning, i18n::text("bookmarks.connect_first"));
            return;
        };
        if !session.listed {
            self.notifications.push(Severity::Warning, i18n::text("sync.wait_loading"));
            return;
        }
        let mut directions =
            vec![("local_to_remote", "sync.dir.local_to_remote"), ("remote_to_local", "sync.dir.remote_to_local")];
        if session.preserves_times {
            directions.push(("both", "sync.dir.both"));
        }
        let fields = vec![
            FormField::choice(DIRECTION, i18n::text("sync.form.direction"), choices(&directions), "local_to_remote"),
            FormField::choice(
                BY,
                i18n::text("sync.form.compare_by"),
                choices(&[("time", "sync.by.time"), ("size", "sync.by.size")]),
                "time",
            ),
            FormField::choice(
                SUBFOLDERS,
                i18n::text("sync.form.subfolders"),
                choices(&[("yes", "sync.sub.yes"), ("no", "sync.sub.no")]),
                "yes",
            ),
            FormField::choice(
                MIRROR,
                i18n::text("sync.form.mirror"),
                choices(&[("no", "sync.mirror_off"), ("yes", "sync.mirror_on")]),
                "no",
            ),
        ];
        let mut form = FormDialog::new(i18n::text_with("sync.form_title", &[("name", &session.name)]), fields);
        if !session.preserves_times {
            form.hint = Some(i18n::text("sync.no_both_hint"));
        }
        let id = session.id;
        self.dialog = Some(Dialog::Form(form));
        self.pending_action = Some(PendingAction::StartSync { session: id });
    }

    pub(super) fn submit_sync_options(&mut self, session: SessionId, values: Vec<(&'static str, String)>) {
        let options = match parse_options(&values) {
            Ok(options) => options,
            Err(message) => {
                self.set_form_error(None, message);
                return;
            }
        };
        let remote_dir = self.sessions.by_id(session).map(|live| live.panel.path().clone());
        self.dialog = None;
        self.pending_action = None;
        let Some(remote_dir) = remote_dir else {
            self.notifications.push(Severity::Warning, i18n::text("sync.connection_gone"));
            return;
        };
        self.core.send(Command::StartSync { session, local_dir: self.local.path().clone(), remote_dir, options });
    }

    pub(super) fn apply_sync_plan(&mut self, plan: Arc<SyncPlan>) {
        if let Some(open_id) = self.sync_view.sync_id() {
            if open_id != plan.sync_id {
                self.core.send(Command::CancelSync { sync_id: plan.sync_id });
                self.notifications.push(Severity::Warning, i18n::text("sync.preview_open"));
            }
            return;
        }
        self.sync_view.load(plan);
        let free_to_open =
            self.screen == Screen::Files && self.dialog.is_none() && !self.help_visible && !self.editing_filter();
        if free_to_open {
            self.screen = Screen::Sync;
        } else {
            self.notifications.push(Severity::Info, i18n::text("sync.preview_ready"));
        }
    }

    pub(super) fn withdraw_sync(&mut self, sync_ids: &[u64]) {
        if self.sync_view.sync_id().is_some_and(|id| sync_ids.contains(&id)) {
            self.sync_view.clear();
            if self.screen == Screen::Sync {
                self.screen = Screen::Files;
            }
            self.notifications.push(Severity::Info, i18n::text("sync.plan_cancelled"));
        }
    }

    pub(super) fn apply_sync_action(&mut self, action: Action) {
        match action {
            Action::Up => self.sync_view.move_cursor(-1),
            Action::Down => self.sync_view.move_cursor(1),
            Action::PageUp => self.sync_view.page_up(),
            Action::PageDown => self.sync_view.page_down(),
            Action::First => self.sync_view.cursor_to_first(),
            Action::Last => self.sync_view.cursor_to_last(),
            Action::ToggleSelect => self.sync_view.toggle(),
            Action::Open => self.run_sync_plan(),
            _ => {}
        }
    }

    pub(super) fn apply_sync_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('a') => self.sync_view.tick_all(),
            KeyCode::Char('n') => self.sync_view.untick_all(),
            KeyCode::Char('f') => self.sync_view.flip(),
            _ => {}
        }
    }

    fn run_sync_plan(&mut self) {
        let Some(sync_id) = self.sync_view.sync_id() else {
            return;
        };
        let choices = self.sync_view.visible_choices();
        if choices.is_empty() {
            let message = if self.sync_view.filter().is_some() {
                i18n::text("sync.nothing_ticked_shown")
            } else {
                i18n::text("sync.nothing_ticked")
            };
            self.notifications.push(Severity::Warning, message);
            return;
        }
        let (files, folders) = self.sync_view.plan().map_or((0, 0), |plan| plan.deletion_counts(&choices));
        if files + folders > 0 {
            let place = self.sync_delete_place();
            let message = mirror_delete_prompt(files, folders, &place);
            self.dialog = Some(Dialog::Confirm(ConfirmDialog::new(message)));
            self.pending_action = Some(PendingAction::ConfirmSyncDelete { sync_id, choices });
            return;
        }
        self.send_sync_run(sync_id, choices);
    }

    pub(super) fn send_sync_run(&mut self, sync_id: u64, choices: Vec<(u32, SyncAction)>) {
        self.core.send(Command::RunSync { sync_id, choices });
        self.sync_view.clear();
        self.screen = Screen::Files;
    }

    fn sync_delete_place(&self) -> String {
        let Some(plan) = self.sync_view.plan() else {
            return String::new();
        };
        match plan.options.direction {
            SyncDirection::RemoteToLocal => this_computer(),
            _ => self.sessions.by_id(plan.session).map(|session| session.name.clone()).unwrap_or_default(),
        }
    }

    pub(super) fn leave_sync_screen(&mut self) {
        if let Some(sync_id) = self.sync_view.sync_id() {
            self.core.send(Command::CancelSync { sync_id });
        }
        self.sync_view.clear();
        self.screen = Screen::Files;
    }
}

fn unknown_choice(what: &str, value: &str) -> String {
    format!("Unknown {what} \"{value}\"")
}

fn parse_options(values: &[(&'static str, String)]) -> Result<SyncOptions, String> {
    let value =
        |key: &str| values.iter().find(|(name, _)| *name == key).map(|(_, text)| text.as_str()).unwrap_or_default();
    let direction = match value(DIRECTION) {
        "local_to_remote" => SyncDirection::LocalToRemote,
        "remote_to_local" => SyncDirection::RemoteToLocal,
        "both" => SyncDirection::Both,
        other => return Err(unknown_choice("direction", other)),
    };
    let by = match value(BY) {
        "time" => SyncBy::Time,
        "size" => SyncBy::Size,
        other => return Err(unknown_choice("comparison", other)),
    };
    let subfolders = match value(SUBFOLDERS) {
        "yes" => true,
        "no" => false,
        other => return Err(unknown_choice("subfolders option", other)),
    };
    let mirror = match value(MIRROR) {
        "yes" => true,
        "no" => false,
        other => return Err(unknown_choice("mirror option", other)),
    };
    let options = SyncOptions { direction, by, subfolders, mirror };
    options.validate()?;
    Ok(options)
}

#[cfg(test)]
mod tests;
