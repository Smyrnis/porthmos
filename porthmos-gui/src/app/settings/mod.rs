use std::path::PathBuf;

use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::Id;
use iced::{Size, Task, window};
use porthmos_core::{Command, ReopenMode, UpdateMode};

use super::{App, Message};
use crate::config_file::{self, Edit};
use crate::settings_view::{GuiSettings, SettingsMessage, SettingsState, editor_from_text};
use crate::shortcuts::{Action, AssignError, KeyPress, chord_from_press, effective};
use crate::windows::{SETTINGS_WINDOW_SIZE, WindowKind};

const RESTART_NOTE: &str = "Saved. Takes effect the next time Porthmos starts.";
const APPLIED_NOTE: &str = "Saved.";
const RESERVED_TEXT: &str = "That shortcut is reserved by the system.";
const NO_CONFIG_TEXT: &str = "The location of the configuration file is unknown.";

fn plain_title(action: Action) -> &'static str {
    action.title().trim_end_matches('\u{2026}')
}

impl App {
    pub fn with_settings(mut self, settings: GuiSettings, config_file: PathBuf) -> App {
        self.gui_settings = settings;
        self.config_file = Some(config_file);
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
            self.show_settings_message(Some(SettingsMessage::Error(NO_CONFIG_TEXT.to_string())));
            return false;
        };
        match config_file::save(&path, &edit) {
            Ok(()) => true,
            Err(message) => {
                self.show_settings_message(Some(SettingsMessage::Error(message)));
                false
            }
        }
    }

    fn core_setting_saved(&mut self, edit: Edit, apply: impl FnOnce(&mut GuiSettings)) {
        if self.save_setting(edit) {
            apply(&mut self.gui_settings);
            self.show_settings_message(Some(SettingsMessage::Note(RESTART_NOTE.to_string())));
        }
    }

    fn assign_captured(&mut self, action: Action, press: &KeyPress) {
        let Some(chord) = chord_from_press(press) else {
            return;
        };
        let before = self.shortcuts.clone();
        match self.shortcuts.assign(action, chord.clone()) {
            Err(AssignError::Reserved) => {
                self.show_settings_message(Some(SettingsMessage::Error(RESERVED_TEXT.to_string())));
            }
            Err(AssignError::UsedBy(owner)) => {
                let text = format!("Already used by {}", plain_title(owner));
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
                    self.show_settings_message(Some(SettingsMessage::Note(APPLIED_NOTE.to_string())));
                }
            }
            Message::SettingsAutoUploadToggled(on) => {
                self.core_setting_saved(Edit::SetAutoUpload(on), |settings| settings.auto_upload = on);
            }
            Message::SettingsSearchPreviewToggled(on) => {
                if self.save_setting(Edit::SetSearchPreview(on)) {
                    self.gui_settings.search_preview = on;
                    self.apply_search_preview_setting(on);
                    self.show_settings_message(Some(SettingsMessage::Note(APPLIED_NOTE.to_string())));
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
                    self.show_settings_message(Some(SettingsMessage::Note(APPLIED_NOTE.to_string())));
                }
            }
            Message::SettingsClosed => return self.close_settings(),
            _ => {}
        }
        Task::none()
    }

    pub fn reopen_choices() -> [(ReopenMode, &'static str); 3] {
        [
            (ReopenMode::Off, "Don't reopen"),
            (ReopenMode::Ask, "Offer to reopen"),
            (ReopenMode::Auto, "Reopen automatically"),
        ]
    }

    pub fn update_mode_choices() -> [(UpdateMode, &'static str); 3] {
        [(UpdateMode::Auto, "Automatic"), (UpdateMode::Notify, "Notify only"), (UpdateMode::Off, "Never")]
    }
}

#[cfg(test)]
mod tests;
