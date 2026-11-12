use iced::widget::{checkbox, column, container, row, scrollable, space, text_input};
use iced::{Alignment, Border, Element, Font, Length, Size};

use super::button::{Look, text_button};
use super::{bold_label, hairline_across, label, wrapped_label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::settings_view::{SettingsMessage, SettingsState};
use crate::shortcuts::{Action, Shortcuts};
use crate::theme::Palette;

pub const SETTINGS_CHROME: f32 = 140.0;
const UNBOUND: &str = "\u{2014}";
const CAPTURE_HINT: &str = "Press the new shortcut, Esc to cancel";
const EDITOR_PLACEHOLDER: &str = "Editor command, for example: code --wait";
const TERMINAL_PLACEHOLDER: &str = "Terminal command, for example: kitty";
const TERMINAL_HELP: &str = "Used by Open terminal here. Leave empty to use $TERMINAL or a terminal found on this system. Put {command} where the command to run belongs if the program needs it elsewhere.";
const THEME_HELP: &str = "Colours you set in theme.toml are kept and applied on top of the theme you pick. The terminal app reads theme.toml when it starts.";
const EDITOR_HELP: &str = "Leave empty to use $VISUAL, $EDITOR or the system default.";

pub fn reset_enabled(shortcuts: &Shortcuts, action: Action) -> bool {
    shortcuts.is_modified(action)
}

pub fn shortcut_text(shortcuts: &Shortcuts, action: Action) -> String {
    let labels: Vec<String> = shortcuts.chords_for(action).iter().map(|chord| chord.label()).collect();
    if labels.is_empty() { UNBOUND.to_string() } else { labels.join(", ") }
}

fn section<'a>(title: &str, palette: &Palette) -> Element<'a, Message> {
    column![space().height(Length::Fixed(6.0)), bold_label(title.to_string(), palette.fg_bright)].into()
}

fn shortcut_row<'a>(app: &App, state: &SettingsState, action: Action) -> Element<'a, Message> {
    let palette = app.palette();
    let capturing = state.capturing == Some(action);
    let current = if capturing {
        label(CAPTURE_HINT, palette.accent)
    } else {
        label(shortcut_text(app.shortcuts(), action), palette.fg_dim)
    };
    let change_look = if capturing { Look::Primary } else { Look::Normal };
    let reset = reset_enabled(app.shortcuts(), action).then_some(Message::SettingsResetPressed(action));
    row![
        container(label(action.title().to_string(), palette.fg)).width(Length::FillPortion(3)).clip(true),
        container(current).width(Length::FillPortion(3)).clip(true),
        text_button("Change", Some(Message::SettingsChangePressed(action)), change_look, palette),
        text_button("Reset", reset, Look::Normal, palette),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .height(Length::Fixed(ROW_HEIGHT + 8.0))
    .into()
}

fn update_choices<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let current = app.gui_settings().update_mode;
    let mut line = row![].spacing(8);
    for (mode, title) in App::update_mode_choices() {
        let look = if mode == current { Look::Chosen } else { Look::Normal };
        line = line.push(text_button(title, Some(Message::SettingsUpdateModeChosen(mode)), look, palette));
    }
    line.into()
}

fn reopen_choices<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let current = app.gui_settings().reopen;
    let mut line = row![].spacing(8);
    for (mode, title) in App::reopen_choices() {
        let look = if mode == current { Look::Chosen } else { Look::Normal };
        line = line.push(text_button(title, Some(Message::SettingsReopenChosen(mode)), look, palette));
    }
    line.into()
}

fn theme_choices<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let current = app.gui_settings().theme_base;
    let mut line = row![].spacing(8);
    for (base, title) in App::theme_choices() {
        let look = if base == current { Look::Chosen } else { Look::Normal };
        line = line.push(text_button(title, Some(Message::SettingsThemeChosen(base)), look, palette));
    }
    line.into()
}

fn appearance_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section("Appearance", palette),
        label("Colour theme:", palette.fg),
        theme_choices(app),
        wrapped_label(THEME_HELP, palette.fg_dim),
    ]
    .spacing(8)
    .into()
}

fn startup_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section("Startup", palette),
        label("Connections that were open when you last quit:", palette.fg),
        reopen_choices(app),
        wrapped_label("Only saved connections are remembered, and only while this is on.", palette.fg_dim),
    ]
    .spacing(8)
    .into()
}

fn connection_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    let reconnect = checkbox(app.gui_settings().reconnect)
        .label("Reconnect automatically when a connection drops")
        .on_toggle(Message::SettingsReconnectToggled)
        .style(crate::view::checkbox::style(palette));
    column![
        section("Connections", palette),
        reconnect,
        wrapped_label(
            "Tries up to five times, then asks you to connect by hand. Stops at once if a password is refused.",
            palette.fg_dim
        ),
    ]
    .spacing(8)
    .into()
}

fn restart_line<'a>(palette: &Palette) -> Element<'a, Message> {
    wrapped_label("Takes effect the next time Porthmos starts.", palette.fg_dim).into()
}

fn editing_section<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input(EDITOR_PLACEHOLDER, &state.editor_text)
        .style(crate::view::input::style(palette))
        .id(App::settings_editor_id())
        .on_input(Message::SettingsEditorTyped)
        .on_submit(Message::SettingsEditorApplied)
        .size(13.0)
        .font(Font::DEFAULT);
    let auto_upload = checkbox(app.gui_settings().auto_upload)
        .label("Upload edited files without asking")
        .on_toggle(Message::SettingsAutoUploadToggled)
        .style(crate::view::checkbox::style(palette));
    column![
        section("Editing", palette),
        row![input, text_button("Apply", Some(Message::SettingsEditorApplied), Look::Normal, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
        wrapped_label(EDITOR_HELP, palette.fg_dim),
        auto_upload,
        restart_line(palette),
    ]
    .spacing(8)
    .into()
}

fn search_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    let preview = checkbox(app.gui_settings().search_preview)
        .label("Show a preview of the file under the cursor")
        .on_toggle(Message::SettingsSearchPreviewToggled)
        .style(crate::view::checkbox::style(palette));
    column![section("Search", palette), preview].spacing(8).into()
}

fn terminal_section<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input(TERMINAL_PLACEHOLDER, &state.terminal_text)
        .style(crate::view::input::style(palette))
        .id(App::settings_terminal_id())
        .on_input(Message::SettingsTerminalTyped)
        .on_submit(Message::SettingsTerminalApplied)
        .size(13.0)
        .font(Font::DEFAULT);
    column![
        section("Terminal", palette),
        row![input, text_button("Apply", Some(Message::SettingsTerminalApplied), Look::Normal, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
        wrapped_label(TERMINAL_HELP, palette.fg_dim),
    ]
    .spacing(8)
    .into()
}

fn updates_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section("Updates", palette),
        label("Check for new versions:", palette.fg),
        update_choices(app),
        restart_line(palette),
    ]
    .spacing(8)
    .into()
}

fn message_line<'a>(state: &SettingsState, palette: &Palette) -> Element<'a, Message> {
    let line = match &state.message {
        Some(SettingsMessage::Error(text)) => label(text.clone(), palette.err),
        Some(SettingsMessage::Note(text)) => label(text.clone(), palette.fg_dim),
        None => label(String::new(), palette.fg_dim),
    };
    container(line).width(Length::Fill).clip(true).into()
}

pub fn window_list_height(window: Size) -> f32 {
    (window.height - SETTINGS_CHROME).max(ROW_HEIGHT)
}

pub fn window(app: &App) -> Option<Element<'_, Message>> {
    let state = app.settings_state()?;
    let palette = app.palette();
    let surface = palette.surface;
    let content = container(body(app, state)).padding(16).width(Length::Fill).height(Length::Fill).style(move |_| {
        container::Style { background: Some(iced::Background::Color(surface)), ..container::Style::default() }
    });
    Some(content.into())
}

fn body<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let mut shortcuts = column![section("Keyboard shortcuts", palette)].spacing(2);
    for action in Action::all() {
        shortcuts = shortcuts.push(shortcut_row(app, state, action));
    }
    let content = column![
        shortcuts,
        appearance_section(app),
        editing_section(app, state),
        search_section(app),
        startup_section(app),
        connection_section(app),
        terminal_section(app, state),
        updates_section(app)
    ]
    .spacing(16)
    .padding([0, 12]);
    let list = scrollable(content)
        .id(App::settings_list_id())
        .width(Length::Fill)
        .height(Length::Fixed(window_list_height(app.settings_window_size())));
    let border = Border { color: palette.line, width: 1.0, radius: 4.0.into() };
    let boxed = container(list).style(move |_| container::Style { border, ..container::Style::default() });
    column![
        bold_label("Settings", palette.fg_bright),
        boxed,
        message_line(state, palette),
        hairline_across(palette),
        row![space().width(Length::Fill), text_button("Close", Some(Message::SettingsClosed), Look::Primary, palette)]
            .spacing(8),
    ]
    .spacing(10)
    .into()
}

#[cfg(test)]
mod tests;
