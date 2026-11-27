use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use std::path::PathBuf;

use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::Id;
use iced::widget::operation::{self, RelativeOffset};
use iced::{Size, Task, window};
use porthmos_core::theme::{self, ThemeBase};
use porthmos_core::{Command, ReopenMode, UpdateMode};

use super::{App, Message};
use crate::refresh;
use crate::settings_view::{
    GuiSettings, LogsField, SettingsMessage, SettingsState, compare_limit_from_text, editor_from_text,
    logs_initial_lines_from_text, logs_max_lines_from_text, logs_range_text, refresh_seconds_from_text,
    search_depth_from_text, search_results_from_text,
};
use crate::shortcuts::{Action, AssignError, KeyPress, chord_from_press, effective};
use crate::theme::{Palette, ThemeSettings};
use crate::windows::{SETTINGS_WINDOW_SIZE, WindowKind};
use porthmos_core::config::file_edit::{self, Edit};

fn restart_note() -> String {
    tr("Saved. Takes effect the next time Porthmos starts.")
}
fn applied_note() -> String {
    tr("Saved.")
}
fn reserved_text() -> String {
    tr("That shortcut is reserved by the system.")
}
fn compare_limit_text() -> String {
    tr("Enter a whole number from 1 to 64.")
}
fn search_depth_text() -> String {
    tr("Enter a whole number from 1 to 64.")
}
fn search_results_text() -> String {
    tr("Enter a whole number from 10 to 10000.")
}
fn refresh_seconds_text() -> String {
    tr("Enter a whole number from 5 to 300.")
}
fn no_config_text() -> String {
    tr("The location of the configuration file is unknown.")
}

fn plain_title(action: Action) -> String {
    action.title().trim_end_matches('\u{2026}').to_string()
}

impl App {
    pub fn with_settings(mut self, settings: GuiSettings, config_file: PathBuf) -> App {
        self.gui_settings = settings;
        self.config_file = Some(config_file);
        self
    }

    pub fn with_theme_accent(mut self, accent: Option<theme::Rgb>) -> App {
        self.theme_accent = accent;
        self
    }

    pub fn gui_settings(&self) -> &GuiSettings {
        &self.gui_settings
    }

    pub fn settings_terminal_id() -> Id {
        Id::new("settings-terminal")
    }

    pub fn settings_editor_id() -> Id {
        Id::new("settings-editor")
    }

    pub fn settings_list_id() -> Id {
        Id::new("settings-list")
    }

    pub fn settings_state(&self) -> Option<&SettingsState> {
        self.settings_state.as_ref()
    }

    #[cfg(test)]
    pub fn settings_open(&self) -> bool {
        self.settings_state.is_some()
    }

    pub fn settings_window_size(&self) -> Size {
        self.windows.id_of(WindowKind::Settings).and_then(|id| self.windows.size_of(id)).unwrap_or(SETTINGS_WINDOW_SIZE)
    }

    pub(in crate::app) fn open_settings(&mut self) -> Task<Message> {
        if let Some(id) = self.windows.id_of(WindowKind::Settings) {
            return window::gain_focus(id);
        }
        if self.modals.front().is_some() {
            return Task::none();
        }
        let mut opening = Task::none();
        self.windows.open(WindowKind::Settings, |settings| {
            let (id, task) = window::open(settings);
            opening = task.map(Message::WindowOpened);
            id
        });
        self.settings_state = Some(SettingsState::new(&self.gui_settings));
        opening
    }

    pub(in crate::app) fn close_settings(&mut self) -> Task<Message> {
        self.settings_state = None;
        match self.windows.id_of(WindowKind::Settings) {
            Some(id) => {
                self.windows.close(id);
                window::close(id)
            }
            None => Task::none(),
        }
    }

    fn settings_state_mut(&mut self) -> Option<&mut SettingsState> {
        self.settings_state.as_mut()
    }

    fn show_settings_message(&mut self, message: Option<SettingsMessage>) {
        if let Some(state) = self.settings_state_mut() {
            state.message = message;
        }
    }

    fn save_setting(&mut self, edit: Edit) -> bool {
        let Some(path) = self.config_file.clone() else {
            self.show_settings_message(Some(SettingsMessage::Error(no_config_text())));
            return false;
        };
        match file_edit::save(&path, &edit) {
            Ok(()) => true,
            Err(message) => {
                self.show_settings_message(Some(SettingsMessage::Error(message)));
                false
            }
        }
    }

    fn theme_file(&self) -> Option<PathBuf> {
        self.config_file.as_ref().map(|config| config.with_file_name(theme::file::FILE_NAME))
    }

    fn choose_theme(&mut self, base: ThemeBase) {
        let Some(path) = self.theme_file() else {
            self.show_settings_message(Some(SettingsMessage::Error(no_config_text())));
            return;
        };
        if let Err(message) = file_edit::save(&path, &Edit::SetThemeBase(base)) {
            self.show_settings_message(Some(SettingsMessage::Error(message)));
            return;
        }
        let (scheme, warnings) = theme::file::load_from(&path, self.theme_accent);
        self.palette = Palette::new(&ThemeSettings::default().with_scheme(scheme));
        self.gui_settings.theme_base = base;
        let message = match warnings.first() {
            Some(warning) => SettingsMessage::Note(warning.clone()),
            None => SettingsMessage::Note(applied_note()),
        };
        self.show_settings_message(Some(message));
    }

    fn core_setting_saved(&mut self, edit: Edit, apply: impl FnOnce(&mut GuiSettings)) {
        if self.save_setting(edit) {
            apply(&mut self.gui_settings);
            self.show_settings_message(Some(SettingsMessage::Note(restart_note())));
        }
    }

    fn assign_captured(&mut self, action: Action, press: &KeyPress) {
        let Some(chord) = chord_from_press(press) else {
            return;
        };
        let before = self.shortcuts.clone();
        match self.shortcuts.assign(action, chord.clone()) {
            Err(AssignError::Reserved) => {
                self.show_settings_message(Some(SettingsMessage::Error(reserved_text())));
            }
            Err(AssignError::UsedBy(owner)) => {
                let text = i18n::text_with("gui.settings.already_used", &[("name", &plain_title(owner))]);
                self.show_settings_message(Some(SettingsMessage::Error(text)));
            }
            Ok(()) => {
                let edit = Edit::SetKey { action: action.name().to_string(), chords: vec![chord.spec()] };
                if self.save_setting(edit) {
                    if let Some(state) = self.settings_state_mut() {
                        state.capturing = None;
                        state.message = None;
                    }
                } else {
                    self.shortcuts = before;
                }
            }
        }
    }

    fn reset_shortcut(&mut self, action: Action) {
        let before = self.shortcuts.clone();
        self.shortcuts.reset(action);
        if self.save_setting(Edit::ResetKey { action: action.name().to_string() }) {
            self.show_settings_message(None);
        } else {
            self.shortcuts = before;
        }
    }

    pub(in crate::app) fn settings_key(&mut self, press: &KeyPress) -> Task<Message> {
        let Some(state) = self.settings_state.as_ref() else {
            return Task::none();
        };
        let capturing = state.capturing;
        if let Some(action) = capturing {
            if press.key == Key::Named(Named::Escape) {
                if let Some(state) = self.settings_state_mut() {
                    state.capturing = None;
                    state.message = None;
                }
            } else {
                self.assign_captured(action, press);
            }
            return Task::none();
        }
        let (key, modifiers) = effective(press);
        if press.key == Key::Named(Named::Escape)
            || self.shortcuts.action_for(&key, modifiers) == Some(Action::Settings)
        {
            return self.close_settings();
        }
        Task::none()
    }

    pub(in crate::app) fn settings_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SettingsChangePressed(action) => {
                if let Some(state) = self.settings_state_mut() {
                    state.capturing = Some(action);
                    state.message = None;
                }
            }
            Message::SettingsResetPressed(action) => self.reset_shortcut(action),
            Message::SettingsEditorTyped(text) => {
                if let Some(state) = self.settings_state_mut() {
                    state.editor_text = text;
                }
            }
            Message::SettingsEditorApplied => {
                let Some(state) = self.settings_state.as_ref() else {
                    return Task::none();
                };
                let editor = editor_from_text(&state.editor_text);
                self.core_setting_saved(Edit::SetEditor(editor.clone()), |settings| settings.editor = editor);
            }
            Message::SettingsCompareLimitTyped(text) => {
                if let Some(state) = self.settings_state_mut() {
                    state.compare_limit_text = text;
                }
            }
            Message::SettingsCompareLimitApplied => {
                let Some(state) = self.settings_state.as_ref() else {
                    return Task::none();
                };
                match compare_limit_from_text(&state.compare_limit_text) {
                    Some(mib) => {
                        self.core_setting_saved(Edit::SetCompareLimitMib(mib), |settings| {
                            settings.compare_limit_mib = mib
                        });
                    }
                    None => self.show_settings_message(Some(SettingsMessage::Error(compare_limit_text()))),
                }
            }
            Message::SettingsSearchDepthTyped(text) => {
                if let Some(state) = self.settings_state_mut() {
                    state.search_depth_text = text;
                }
            }
            Message::SettingsSearchDepthApplied => {
                let Some(state) = self.settings_state.as_ref() else {
                    return Task::none();
                };
                match search_depth_from_text(&state.search_depth_text) {
                    Some(depth) => {
                        self.core_setting_saved(Edit::SetSearchMaxDepth(depth as u64), |settings| {
                            settings.search_max_depth = depth
                        });
                    }
                    None => self.show_settings_message(Some(SettingsMessage::Error(search_depth_text()))),
                }
            }
            Message::SettingsSearchResultsTyped(text) => {
                if let Some(state) = self.settings_state_mut() {
                    state.search_results_text = text;
                }
            }
            Message::SettingsSearchResultsApplied => {
                let Some(state) = self.settings_state.as_ref() else {
                    return Task::none();
                };
                match search_results_from_text(&state.search_results_text) {
                    Some(count) => {
                        self.core_setting_saved(Edit::SetSearchMaxResults(count as u64), |settings| {
                            settings.search_max_results = count
                        });
                    }
                    None => self.show_settings_message(Some(SettingsMessage::Error(search_results_text()))),
                }
            }
            Message::SettingsTerminalTyped(text) => {
                if let Some(state) = self.settings_state_mut() {
                    state.terminal_text = text;
                }
            }
            Message::SettingsTerminalApplied => {
                let Some(state) = self.settings_state.as_ref() else {
                    return Task::none();
                };
                let terminal = editor_from_text(&state.terminal_text);
                if self.save_setting(Edit::SetTerminal(terminal.clone())) {
                    self.gui_settings.terminal = terminal;
                    self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                }
            }
            Message::SettingsEmbeddedTerminalToggled(on) => {
                if self.save_setting(Edit::SetEmbeddedTerminal(on)) {
                    self.gui_settings.embedded_terminal = on;
                    self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                }
            }
            Message::SettingsAutoUploadToggled(on) => {
                self.core_setting_saved(Edit::SetAutoUpload(on), |settings| settings.auto_upload = on);
            }
            Message::SettingsSearchPreviewToggled(on) => {
                if self.save_setting(Edit::SetSearchPreview(on)) {
                    self.gui_settings.search_preview = on;
                    self.apply_search_preview_setting(on);
                    self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                }
            }
            Message::SettingsLanguageChosen(code) => {
                if code.as_deref().is_some_and(|code| !i18n::is_available(code)) {
                    return Task::none();
                }
                if self.save_setting(Edit::SetLanguage(code.clone())) {
                    i18n::use_language(code.as_deref());
                    self.gui_settings.language = code;
                    self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                }
            }
            Message::SettingsUpdateModeChosen(mode) => {
                self.core_setting_saved(Edit::SetUpdateMode(mode), |settings| settings.update_mode = mode);
            }
            Message::SettingsReopenChosen(mode) => {
                if self.save_setting(Edit::SetReopenMode(mode)) {
                    self.gui_settings.reopen = mode;
                    self.core.send(Command::SetReopenMode { mode });
                    if mode == ReopenMode::Off {
                        self.reopen_offer.clear();
                    }
                    self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                }
            }
            Message::SettingsReconnectToggled(on) => {
                if self.save_setting(Edit::SetReconnect(on)) {
                    self.gui_settings.reconnect = on;
                    self.core.send(Command::SetReconnect { enabled: on });
                    self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                }
            }
            Message::SettingsRefreshToggled(tab, auto) => {
                if self.save_setting(Edit::SetRefreshAuto { tab, auto }) {
                    refresh::setting_mut(&mut self.gui_settings.refresh, tab).auto = auto;
                    self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                }
            }
            Message::SettingsRefreshTyped(tab, text) => {
                if let Some(state) = self.settings_state_mut() {
                    state.refresh_text[refresh::index(tab)] = text;
                }
            }
            Message::SettingsRefreshApplied(tab) => {
                let Some(state) = self.settings_state.as_ref() else {
                    return Task::none();
                };
                match refresh_seconds_from_text(&state.refresh_text[refresh::index(tab)]) {
                    Some(seconds) => {
                        if self.save_setting(Edit::SetRefreshInterval { tab, seconds }) {
                            refresh::setting_mut(&mut self.gui_settings.refresh, tab).interval_seconds = seconds;
                            self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                        }
                    }
                    None => self.show_settings_message(Some(SettingsMessage::Error(refresh_seconds_text()))),
                }
            }
            Message::SettingsLogsTyped(field, text) => {
                if let Some(state) = self.settings_state_mut() {
                    match field {
                        LogsField::MaxLines => state.logs_max_text = text,
                        LogsField::InitialLines => state.logs_initial_text = text,
                    }
                }
            }
            Message::SettingsLogsApplied(field) => {
                let Some(state) = self.settings_state.as_ref() else {
                    return Task::none();
                };
                let applied = match field {
                    LogsField::MaxLines => logs_max_lines_from_text(&state.logs_max_text)
                        .map(|lines| (Edit::SetLogsMaxLines(lines), lines)),
                    LogsField::InitialLines => logs_initial_lines_from_text(&state.logs_initial_text)
                        .map(|lines| (Edit::SetLogsInitialLines(lines), lines)),
                };
                match applied {
                    Some((edit, lines)) => {
                        if self.save_setting(edit) {
                            match field {
                                LogsField::MaxLines => {
                                    self.gui_settings.logs.max_lines = lines;
                                    self.logs.trim(lines as usize);
                                }
                                LogsField::InitialLines => self.gui_settings.logs.initial_lines = lines,
                            }
                            self.show_settings_message(Some(SettingsMessage::Note(applied_note())));
                        }
                    }
                    None => self.show_settings_message(Some(SettingsMessage::Error(logs_range_text(field)))),
                }
            }
            Message::SettingsThemeChosen(base) => self.choose_theme(base),
            Message::SettingsCategoryChosen(category) => {
                let Some(state) = self.settings_state_mut() else {
                    return Task::none();
                };
                state.category = category;
                state.capturing = None;
                state.message = None;
                return operation::snap_to(App::settings_list_id(), RelativeOffset::START);
            }
            Message::SettingsClosed => return self.close_settings(),
            _ => {}
        }
        Task::none()
    }

    pub fn reopen_choices() -> [(ReopenMode, String); 3] {
        [
            (ReopenMode::Off, tr("Don't reopen")),
            (ReopenMode::Ask, tr("Offer to reopen")),
            (ReopenMode::Auto, tr("Reopen automatically")),
        ]
    }

    pub fn language_choices() -> Vec<(Option<String>, String)> {
        let mut choices = vec![(None, tr("Automatic"))];
        choices.extend(
            i18n::available_languages()
                .iter()
                .map(|language| (Some(language.code.to_string()), language.name.to_string())),
        );
        choices
    }

    pub fn theme_choices() -> [(ThemeBase, String); 3] {
        [(ThemeBase::Dark, tr("Dark")), (ThemeBase::HighContrast, tr("High contrast")), (ThemeBase::Light, tr("Light"))]
    }

    pub fn update_mode_choices() -> [(UpdateMode, String); 3] {
        [(UpdateMode::Auto, tr("Automatic")), (UpdateMode::Notify, tr("Notify only")), (UpdateMode::Off, tr("Never"))]
    }
}

#[cfg(test)]
mod tests;
