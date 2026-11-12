use iced::widget::{checkbox, column, container, row, scrollable, text_input};
use iced::{Alignment, Element, Font, Length, Size};

use super::button::{Look, nav_item, text_button};
use super::spacing::{LG, MD, XL};
use super::{bold_label, hairline_across, hairline_down, label, wrapped_label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::settings_view::{SettingsCategory, SettingsMessage, SettingsSection, SettingsState};
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
    column![bold_label(title.to_string(), palette.fg_bright), hairline_across(palette)].spacing(8).into()
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
        .text_size(13.0)
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
        .text_size(13.0)
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
        .text_size(13.0)
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
    let ground = palette.bg;
    let content = container(body(app, state)).padding(16).width(Length::Fill).height(Length::Fill).style(move |_| {
        container::Style { background: Some(iced::Background::Color(ground)), ..container::Style::default() }
    });
    Some(content.into())
}

const SIDEBAR_WIDTH: f32 = 170.0;

fn section_view<'a>(app: &'a App, state: &'a SettingsState, which: SettingsSection) -> Element<'a, Message> {
    let palette = app.palette();
    match which {
        SettingsSection::Shortcuts => {
            let mut shortcuts = column![section("Keyboard shortcuts", palette)].spacing(2);
            for action in Action::all() {
                shortcuts = shortcuts.push(shortcut_row(app, state, action));
            }
            shortcuts.into()
        }
        SettingsSection::Appearance => appearance_section(app),
        SettingsSection::Editing => editing_section(app, state),
        SettingsSection::Terminal => terminal_section(app, state),
        SettingsSection::Search => search_section(app),
        SettingsSection::Startup => startup_section(app),
        SettingsSection::Connections => connection_section(app),
        SettingsSection::Updates => updates_section(app),
    }
}

fn sidebar<'a>(app: &App, state: &SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let mut entries = column![].spacing(2);
    for category in SettingsCategory::ALL {
        let selected = category == state.category;
        entries =
            entries.push(nav_item(category.title(), selected, Message::SettingsCategoryChosen(category), palette));
    }
    container(entries).width(Length::Fixed(SIDEBAR_WIDTH)).padding([0, 8]).into()
}

fn body<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let mut content = column![].spacing(XL).padding([0.0, LG]);
    for section in state.category.sections() {
        content = content.push(section_view(app, state, *section));
    }
    let list = scrollable(content)
        .style(crate::view::scroll::style(palette))
        .id(App::settings_list_id())
        .width(Length::Fill)
        .height(Length::Fixed(window_list_height(app.settings_window_size())));
    column![
        bold_label("Settings", palette.fg_bright),
        hairline_across(palette),
        row![sidebar(app, state), hairline_down(palette), list].spacing(8),
        hairline_across(palette),
        row![message_line(state, palette), text_button("Close", Some(Message::SettingsClosed), Look::Primary, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
    ]
    .spacing(MD)
    .into()
}

#[cfg(test)]
mod tests;
