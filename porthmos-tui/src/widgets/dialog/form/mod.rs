use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, layout::Rect};

use crate::{
    theme::Theme,
    widgets::{
        form_fields::{LABEL_WIDTH, displayed_value, render_field_stack, stack_height},
        history_view::printable,
        popup::{content_width, footer, footer_text, render_popup},
    },
};

pub const KEPT_SECRET: &str = "\u{0}kept";
const MIN_POPUP_WIDTH: u16 = 44;
const FOOTER_ROWS: u16 = 2;
const BORDER_ROWS: u16 = 2;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FormLayout {
    #[default]
    Popup,
    Screen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Masked,
    SavedSecret,
    Choice { choices: Vec<(String, String)>, selected: usize },
}

pub struct FormField {
    pub key: &'static str,
    pub label: String,
    pub value: String,
    pub cursor: usize,
    pub kind: FieldKind,
}

impl FormField {
    pub fn text(key: &'static str, label: impl Into<String>, initial_value: impl Into<String>) -> Self {
        let value = initial_value.into();
        let cursor = value.chars().count();
        Self { key, label: label.into(), value, cursor, kind: FieldKind::Text }
    }

    pub fn masked(key: &'static str, label: impl Into<String>, initial_value: impl Into<String>) -> Self {
        Self { kind: FieldKind::Masked, ..Self::text(key, label, initial_value) }
    }

    pub fn saved_secret(key: &'static str, label: impl Into<String>) -> Self {
        Self { kind: FieldKind::SavedSecret, ..Self::text(key, label, "") }
    }

    pub fn choice(
        key: &'static str, label: impl Into<String>, choices: Vec<(String, String)>, selected_value: &str,
    ) -> Self {
        let selected = choices.iter().position(|(value, _)| value == selected_value).unwrap_or(0);
        Self {
            key,
            label: label.into(),
            value: String::new(),
            cursor: 0,
            kind: FieldKind::Choice { choices, selected },
        }
    }

    pub fn submitted_value(&self) -> String {
        match &self.kind {
            FieldKind::Choice { choices, selected } => {
                choices.get(*selected).map(|(value, _)| value.clone()).unwrap_or_default()
            }
            FieldKind::SavedSecret => KEPT_SECRET.to_string(),
            FieldKind::Text | FieldKind::Masked => self.value.clone(),
        }
    }

    fn is_choice(&self) -> bool {
        matches!(self.kind, FieldKind::Choice { .. })
    }

    pub fn is_toggle(&self) -> bool {
        match &self.kind {
            FieldKind::Choice { choices, .. } => {
                choices.len() == 2
                    && choices.iter().all(|(value, _)| value == "true" || value == "false")
                    && choices[0].0 != choices[1].0
            }
            _ => false,
        }
    }

    fn shift_choice(&mut self, forward: bool) -> bool {
        let FieldKind::Choice { choices, selected } = &mut self.kind else {
            return false;
        };
        if choices.len() < 2 {
            return false;
        }
        *selected =
            if forward { (*selected + 1) % choices.len() } else { (*selected + choices.len() - 1) % choices.len() };
        true
    }

    fn insert_char_at(&mut self, index: usize, ch: char) {
        let byte_index = self.byte_index_for(index);
        self.value.insert(byte_index, ch);
    }

    fn remove_char_at(&mut self, index: usize) {
        let byte_index = self.byte_index_for(index);
        self.value.remove(byte_index);
    }

    fn byte_index_for(&self, char_index: usize) -> usize {
        self.value.char_indices().nth(char_index).map(|(byte, _)| byte).unwrap_or(self.value.len())
    }
}

pub struct FormDialog {
    pub title: String,
    pub fields: Vec<FormField>,
    pub focused: usize,
    pub error: Option<String>,
    pub hint: Option<String>,
    pub saved_secrets: Option<(String, Vec<String>)>,
    pub remembered: BTreeMap<String, String>,
    pub prefilled: BTreeMap<&'static str, String>,
    pub layout: FormLayout,
}

#[derive(Clone, PartialEq, Eq)]
pub enum FormOutcome {
    Pending,
    Submitted(Vec<(&'static str, String)>),
    ChoiceChanged { key: &'static str },
    Cancelled,
}

pub(super) fn submitted_keys(values: &[(&'static str, String)]) -> Vec<&'static str> {
    values.iter().map(|(key, _)| *key).collect()
}

impl std::fmt::Debug for FormOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormOutcome::Pending => write!(f, "Pending"),
            FormOutcome::Submitted(values) => f.debug_tuple("Submitted").field(&submitted_keys(values)).finish(),
            FormOutcome::ChoiceChanged { key } => f.debug_struct("ChoiceChanged").field("key", key).finish(),
            FormOutcome::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl FormDialog {
    pub fn new(title: impl Into<String>, fields: Vec<FormField>) -> Self {
        Self {
            title: title.into(),
            fields,
            focused: 0,
            error: None,
            hint: None,
            saved_secrets: None,
            remembered: BTreeMap::new(),
            prefilled: BTreeMap::new(),
            layout: FormLayout::Popup,
        }
    }

    pub fn value(&self, key: &str) -> Option<String> {
        self.fields.iter().find(|field| field.key == key).map(FormField::submitted_value)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.submit();
        }
        match self.layout {
            FormLayout::Popup => self.handle_editing_key(key),
            FormLayout::Screen => self.handle_screen_key(key),
        }
    }

    fn submit(&self) -> FormOutcome {
        FormOutcome::Submitted(self.fields.iter().map(|field| (field.key, field.submitted_value())).collect())
    }

    fn flip_focused_toggle(&mut self) -> FormOutcome {
        let field = &mut self.fields[self.focused];
        if field.shift_choice(true) { FormOutcome::ChoiceChanged { key: field.key } } else { FormOutcome::Pending }
    }

    fn handle_screen_key(&mut self, key: KeyEvent) -> FormOutcome {
        let last = self.fields.len() - 1;
        if self.focused == 0 {
            return match key.code {
                KeyCode::Esc => FormOutcome::Cancelled,
                KeyCode::Up | KeyCode::Down => {
                    let field = &mut self.fields[0];
                    if field.shift_choice(key.code == KeyCode::Down) {
                        FormOutcome::ChoiceChanged { key: field.key }
                    } else {
                        FormOutcome::Pending
                    }
                }
                KeyCode::Right | KeyCode::Tab | KeyCode::Enter => {
                    self.focused = 1.min(last);
                    FormOutcome::Pending
                }
                KeyCode::BackTab => {
                    self.focused = last;
                    FormOutcome::Pending
                }
                _ => FormOutcome::Pending,
            };
        }
        match key.code {
            KeyCode::Tab => {
                self.focused = if self.focused >= last { 0 } else { self.focused + 1 };
                FormOutcome::Pending
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused -= 1;
                FormOutcome::Pending
            }
            KeyCode::Down => {
                self.focused = (self.focused + 1).min(last);
                FormOutcome::Pending
            }
            KeyCode::Left if !self.fields[self.focused].is_choice() && self.fields[self.focused].cursor == 0 => {
                self.focused = 0;
                FormOutcome::Pending
            }
            KeyCode::Char(' ') if self.fields[self.focused].is_toggle() => self.flip_focused_toggle(),
            _ => self.handle_editing_key(key),
        }
    }

    fn handle_editing_key(&mut self, key: KeyEvent) -> FormOutcome {
        let on_choice = self.fields[self.focused].is_choice();
        let field = &mut self.fields[self.focused];
        if field.kind == FieldKind::SavedSecret {
            match key.code {
                KeyCode::Char(c) => {
                    field.kind = FieldKind::Masked;
                    field.value = c.to_string();
                    field.cursor = 1;
                    return FormOutcome::Pending;
                }
                KeyCode::Backspace | KeyCode::Delete => {
                    field.kind = FieldKind::Masked;
                    field.value.clear();
                    field.cursor = 0;
                    return FormOutcome::Pending;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Esc => FormOutcome::Cancelled,
            KeyCode::Enter => self.submit(),
            KeyCode::Char(' ') if self.fields[self.focused].is_toggle() => self.flip_focused_toggle(),
            KeyCode::Left | KeyCode::Right if on_choice => {
                let field = &mut self.fields[self.focused];
                if field.shift_choice(key.code == KeyCode::Right) {
                    FormOutcome::ChoiceChanged { key: field.key }
                } else {
                    FormOutcome::Pending
                }
            }
            KeyCode::Home | KeyCode::End | KeyCode::Backspace | KeyCode::Delete | KeyCode::Char(_) if on_choice => {
                FormOutcome::Pending
            }
            KeyCode::Tab | KeyCode::Down => {
                self.focused = (self.focused + 1) % self.fields.len();
                FormOutcome::Pending
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = (self.focused + self.fields.len() - 1) % self.fields.len();
                FormOutcome::Pending
            }
            KeyCode::Left => {
                let field = &mut self.fields[self.focused];
                field.cursor = field.cursor.saturating_sub(1);
                FormOutcome::Pending
            }
            KeyCode::Right => {
                let field = &mut self.fields[self.focused];
                field.cursor = (field.cursor + 1).min(field.value.chars().count());
                FormOutcome::Pending
            }
            KeyCode::Home => {
                self.fields[self.focused].cursor = 0;
                FormOutcome::Pending
            }
            KeyCode::End => {
                let field = &mut self.fields[self.focused];
                field.cursor = field.value.chars().count();
                FormOutcome::Pending
            }
            KeyCode::Backspace => {
                let field = &mut self.fields[self.focused];
                if field.cursor > 0 {
                    field.remove_char_at(field.cursor - 1);
                    field.cursor -= 1;
                }
                FormOutcome::Pending
            }
            KeyCode::Delete => {
                let field = &mut self.fields[self.focused];
                if field.cursor < field.value.chars().count() {
                    field.remove_char_at(field.cursor);
                }
                FormOutcome::Pending
            }
            KeyCode::Char(c) => {
                let field = &mut self.fields[self.focused];
                field.insert_char_at(field.cursor, c);
                field.cursor += 1;
                FormOutcome::Pending
            }
            _ => FormOutcome::Pending,
        }
    }
}

pub fn render_form(frame: &mut Frame, area: Rect, dialog: &FormDialog, theme: &Theme) {
    let title = printable(&dialog.title);
    let mut measured: Vec<String> = dialog
        .fields
        .iter()
        .map(|field| format!("{:<LABEL_WIDTH$}{}", printable(&field.label), displayed_value(field)))
        .collect();
    measured.extend(dialog.hint.iter().map(|hint| printable(hint)));
    measured.extend(dialog.error.iter().map(|error| format!("\u{2022} {}", printable(error))));
    measured.push(title.clone());
    measured.push(footer_text(&footer_pairs(dialog)));
    let measured: Vec<&str> = measured.iter().map(String::as_str).collect();
    let width = content_width(&measured).max(MIN_POPUP_WIDTH).min(area.width);
    let height = stack_height(dialog, 0).saturating_add(FOOTER_ROWS + BORDER_ROWS).min(area.height);

    let inner = render_popup(frame, area, (width, height), Some(&title), theme);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let stack = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(FOOTER_ROWS));
    render_field_stack(frame, stack, dialog, 0, theme);
    footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), &footer_pairs(dialog), theme);
}

fn footer_pairs(dialog: &FormDialog) -> Vec<(&'static str, &'static str)> {
    let mut pairs = Vec::new();
    if dialog.fields.len() > 1 {
        pairs.push(("tab", "next"));
    }
    pairs.push(("enter", "ok"));
    pairs.push(("esc", "cancel"));
    pairs
}

#[cfg(test)]
mod tests;
