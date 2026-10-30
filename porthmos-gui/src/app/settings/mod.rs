use std::path::PathBuf;

use iced::Task;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::Id;
use porthmos_core::UpdateMode;

use super::{App, Message};
use crate::config_file::{self, Edit};
use crate::settings_view::{GuiSettings, SettingsMessage, SettingsState, editor_from_text};
use crate::shortcuts::{Action, AssignError, KeyPress, chord_from_press, effective};
use crate::tools::Tool;

const RESTART_NOTE: &str = "Saved. Takes effect the next time Porthmos starts.";
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

    pub fn settings_editor_id() -> Id {
        Id::new("settings-editor")
    }

    pub(in crate::app) fn open_settings(&mut self) {
        if self.modals.front().is_some() {
            return;
        }
        self.open_tool(Tool::Settings(SettingsState::new(&self.gui_settings)));
    }

    fn settings_state_mut(&mut self) -> Option<&mut SettingsState> {
        match self.tool.as_mut() {
            Some(Tool::Settings(state)) => Some(state),
            _ => None,
        }
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
        let capturing = match self.tool.as_ref() {
            Some(Tool::Settings(state)) => state.capturing,
            _ => return Task::none(),
        };
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
            self.close_tool();
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
                let Some(Tool::Settings(state)) = self.tool.as_ref() else {
                    return Task::none();
                };
                let editor = editor_from_text(&state.editor_text);
                self.core_setting_saved(Edit::SetEditor(editor.clone()), |settings| settings.editor = editor);
            }
            Message::SettingsAutoUploadToggled(on) => {
                self.core_setting_saved(Edit::SetAutoUpload(on), |settings| settings.auto_upload = on);
            }
            Message::SettingsUpdateModeChosen(mode) => {
                self.core_setting_saved(Edit::SetUpdateMode(mode), |settings| settings.update_mode = mode);
            }
            Message::SettingsScrolled(offset) => self.tool_scroll = offset,
            _ => {}
        }
        Task::none()
    }

    pub fn update_mode_choices() -> [(UpdateMode, &'static str); 3] {
        [(UpdateMode::Auto, "Automatic"), (UpdateMode::Notify, "Notify only"), (UpdateMode::Off, "Never")]
    }
}

#[cfg(test)]
mod tests;
