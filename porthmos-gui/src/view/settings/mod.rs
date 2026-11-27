use iced::widget::{checkbox, column, container, row, scrollable, text_input};
use iced::{Alignment, Element, Font, Length, Size};
use porthmos_core::RefreshTab;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;

use super::button::{Look, nav_item, text_button};
use super::spacing::{LG, MD, XL};
use super::{bold_label, hairline_across, hairline_down, label, wrapped_label};
use crate::app::{App, Message};
use crate::geometry::ROW_HEIGHT;
use crate::refresh;
use crate::settings_view::{LogsField, SettingsCategory, SettingsMessage, SettingsSection, SettingsState, logs_bounds};
use crate::shortcuts::{Action, Shortcuts};
use crate::theme::Palette;

pub const SETTINGS_CHROME: f32 = 140.0;
const UNBOUND: &str = "\u{2014}";
fn capture_hint() -> String {
    tr("Press the new shortcut, Esc to cancel")
}
fn editor_placeholder() -> String {
    tr("Editor command, for example: code --wait")
}
fn terminal_placeholder() -> String {
    tr("Terminal command, for example: kitty")
}
fn terminal_help() -> String {
    tr(
        "Used by Open terminal here. Leave empty to use $TERMINAL or a terminal found on this system. Put {command} where the command to run belongs if the program needs it elsewhere.",
    )
}
fn theme_help() -> String {
    tr("Colours you set in theme.toml are kept and applied on top of the theme you pick.")
}
const COMPARE_LIMIT_PLACEHOLDER: &str = "1";
fn compare_limit_label() -> String {
    tr("Largest file to compare, in MiB (1 to 64)")
}
fn compare_limit_help() -> String {
    tr("When an edited file changed on the server, Show differences compares files up to this size.")
}
const REFRESH_PLACEHOLDER: &str = "10";
pub fn refresh_unit_label() -> String {
    tr("seconds")
}
fn refresh_help() -> String {
    tr("Seconds between refreshes (5 to 300).")
}
const COMPARE_LIMIT_FIELD_WIDTH: f32 = 80.0;
const LOGS_FIELD_WIDTH: f32 = 96.0;
const LOGS_MAX_PLACEHOLDER: &str = "20000";
const LOGS_INITIAL_PLACEHOLDER: &str = "500";
const SEARCH_DEPTH_PLACEHOLDER: &str = "16";
const SEARCH_RESULTS_PLACEHOLDER: &str = "1000";
fn search_depth_label() -> String {
    tr("Deepest folder level to search (1 to 64)")
}
fn search_results_label() -> String {
    tr("Most results to show (10 to 10000)")
}
fn search_limits_help() -> String {
    tr(
        "A search stops at these limits and says the results were cut short. Folders such as .git and node_modules are searched too.",
    )
}
fn editor_help() -> String {
    tr("Leave empty to use $VISUAL, $EDITOR or the system default.")
}

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
        label(capture_hint(), palette.accent)
    } else {
        label(shortcut_text(app.shortcuts(), action), palette.fg_dim)
    };
    let change_look = if capturing { Look::Primary } else { Look::Normal };
    let reset = reset_enabled(app.shortcuts(), action).then_some(Message::SettingsResetPressed(action));
    row![
        container(label(action.title().to_string(), palette.fg)).width(Length::FillPortion(3)).clip(true),
        container(current).width(Length::FillPortion(3)).clip(true),
        text_button(&tr("Change"), Some(Message::SettingsChangePressed(action)), change_look, palette),
        text_button(&tr("Reset"), reset, Look::Normal, palette),
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
        line = line.push(text_button(&title, Some(Message::SettingsUpdateModeChosen(mode)), look, palette));
    }
    line.into()
}

fn reopen_choices<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let current = app.gui_settings().reopen;
    let mut line = row![].spacing(8);
    for (mode, title) in App::reopen_choices() {
        let look = if mode == current { Look::Chosen } else { Look::Normal };
        line = line.push(text_button(&title, Some(Message::SettingsReopenChosen(mode)), look, palette));
    }
    line.into()
}

fn theme_choices<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let current = app.gui_settings().theme_base;
    let mut line = row![].spacing(8);
    for (base, title) in App::theme_choices() {
        let look = if base == current { Look::Chosen } else { Look::Normal };
        line = line.push(text_button(&title, Some(Message::SettingsThemeChosen(base)), look, palette));
    }
    line.into()
}

fn appearance_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section(&tr("Appearance"), palette),
        label(tr("Colour theme:"), palette.fg),
        theme_choices(app),
        wrapped_label(theme_help(), palette.fg_dim),
    ]
    .spacing(8)
    .into()
}

fn language_choices<'a>(app: &App) -> Element<'a, Message> {
    let palette = app.palette();
    let current = app.gui_settings().language.clone();
    let mut line = row![].spacing(8);
    for (code, title) in App::language_choices() {
        let look = if code == current { Look::Chosen } else { Look::Normal };
        line = line.push(text_button(&title, Some(Message::SettingsLanguageChosen(code)), look, palette));
    }
    line.into()
}

fn language_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section(&tr("Language"), palette),
        label(tr("Language of the interface:"), palette.fg),
        language_choices(app),
        wrapped_label(tr("Applies at once. Automatic follows the language of the system."), palette.fg_dim),
    ]
    .spacing(8)
    .into()
}

fn startup_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section(&tr("Startup"), palette),
        label(tr("Connections that were open when you last quit:"), palette.fg),
        reopen_choices(app),
        wrapped_label(tr("Only saved connections are remembered, and only while this is on."), palette.fg_dim),
    ]
    .spacing(8)
    .into()
}

fn connection_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    let reconnect = checkbox(app.gui_settings().reconnect)
        .label(tr("Reconnect automatically when a connection drops"))
        .text_size(13.0)
        .on_toggle(Message::SettingsReconnectToggled)
        .style(crate::view::checkbox::style(palette));
    column![
        section(&tr("Connections"), palette),
        reconnect,
        wrapped_label(
            tr("Tries up to five times, then asks you to connect by hand. Stops at once if a password is refused."),
            palette.fg_dim
        ),
    ]
    .spacing(8)
    .into()
}

fn refresh_label(tab: RefreshTab) -> String {
    match tab {
        RefreshTab::Health => tr("Refresh Health automatically"),
        RefreshTab::Services => tr("Refresh Services automatically"),
        RefreshTab::Ports => tr("Refresh Ports automatically"),
    }
}

pub fn refresh_field_enabled(app: &App, tab: RefreshTab) -> bool {
    refresh::setting(&app.gui_settings().refresh, tab).auto
}

fn refresh_row<'a>(app: &'a App, state: &'a SettingsState, tab: RefreshTab) -> Element<'a, Message> {
    let palette = app.palette();
    let enabled = refresh_field_enabled(app, tab);
    let toggle = checkbox(enabled)
        .label(refresh_label(tab))
        .text_size(13.0)
        .on_toggle(move |on| Message::SettingsRefreshToggled(tab, on))
        .style(crate::view::checkbox::style(palette));
    let mut seconds = text_input(REFRESH_PLACEHOLDER, &state.refresh_text[refresh::index(tab)])
        .style(crate::view::input::style(palette))
        .size(13.0)
        .width(Length::Fixed(COMPARE_LIMIT_FIELD_WIDTH))
        .font(Font::DEFAULT);
    if enabled {
        seconds = seconds
            .on_input(move |text| Message::SettingsRefreshTyped(tab, text))
            .on_submit(Message::SettingsRefreshApplied(tab));
    }
    let apply = enabled.then_some(Message::SettingsRefreshApplied(tab));
    column![
        toggle,
        row![
            seconds,
            label(refresh_unit_label(), palette.fg_dim),
            text_button(&tr("Apply"), apply, Look::Normal, palette)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(8)
    .into()
}

fn refresh_section<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let mut rows = column![section(&tr("Automatic refresh"), palette)].spacing(8);
    for tab in refresh::ALL {
        rows = rows.push(refresh_row(app, state, tab));
    }
    rows.push(wrapped_label(refresh_help(), palette.fg_dim)).into()
}

pub fn logs_field_label(field: LogsField) -> String {
    let (min, max) = logs_bounds(field);
    let key = match field {
        LogsField::MaxLines => "gui.settings.logs_max_label",
        LogsField::InitialLines => "gui.settings.logs_initial_label",
    };
    i18n::text_with(key, &[("min", &min.to_string()), ("max", &max.to_string())])
}

fn logs_row<'a>(app: &'a App, state: &'a SettingsState, field: LogsField) -> Element<'a, Message> {
    let palette = app.palette();
    let (shown, placeholder) = match field {
        LogsField::MaxLines => (&state.logs_max_text, LOGS_MAX_PLACEHOLDER),
        LogsField::InitialLines => (&state.logs_initial_text, LOGS_INITIAL_PLACEHOLDER),
    };
    let input = text_input(placeholder, shown)
        .style(crate::view::input::style(palette))
        .size(13.0)
        .width(Length::Fixed(LOGS_FIELD_WIDTH))
        .font(Font::DEFAULT)
        .on_input(move |text| Message::SettingsLogsTyped(field, text))
        .on_submit(Message::SettingsLogsApplied(field));
    column![
        label(logs_field_label(field), palette.fg),
        row![input, text_button(&tr("Apply"), Some(Message::SettingsLogsApplied(field)), Look::Normal, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
    ]
    .spacing(8)
    .into()
}

fn logs_section<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section(&tr("Logs"), palette),
        logs_row(app, state, LogsField::MaxLines),
        logs_row(app, state, LogsField::InitialLines),
    ]
    .spacing(8)
    .into()
}

fn restart_line<'a>(palette: &Palette) -> Element<'a, Message> {
    wrapped_label(tr("Takes effect the next time Porthmos starts."), palette.fg_dim).into()
}

fn editing_section<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input(&editor_placeholder(), &state.editor_text)
        .style(crate::view::input::style(palette))
        .id(App::settings_editor_id())
        .on_input(Message::SettingsEditorTyped)
        .on_submit(Message::SettingsEditorApplied)
        .size(13.0)
        .font(Font::DEFAULT);
    let auto_upload = checkbox(app.gui_settings().auto_upload)
        .label(tr("Upload edited files without asking"))
        .text_size(13.0)
        .on_toggle(Message::SettingsAutoUploadToggled)
        .style(crate::view::checkbox::style(palette));
    let compare_limit = text_input(COMPARE_LIMIT_PLACEHOLDER, &state.compare_limit_text)
        .style(crate::view::input::style(palette))
        .on_input(Message::SettingsCompareLimitTyped)
        .on_submit(Message::SettingsCompareLimitApplied)
        .size(13.0)
        .width(Length::Fixed(COMPARE_LIMIT_FIELD_WIDTH))
        .font(Font::DEFAULT);
    column![
        section(&tr("Editing"), palette),
        row![input, text_button(&tr("Apply"), Some(Message::SettingsEditorApplied), Look::Normal, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
        wrapped_label(editor_help(), palette.fg_dim),
        auto_upload,
        wrapped_label(compare_limit_label(), palette.fg),
        row![
            compare_limit,
            text_button(&tr("Apply"), Some(Message::SettingsCompareLimitApplied), Look::Normal, palette)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        wrapped_label(compare_limit_help(), palette.fg_dim),
        restart_line(palette),
    ]
    .spacing(8)
    .into()
}

fn search_section<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let preview = checkbox(app.gui_settings().search_preview)
        .label(tr("Show a preview of the file under the cursor"))
        .text_size(13.0)
        .on_toggle(Message::SettingsSearchPreviewToggled)
        .style(crate::view::checkbox::style(palette));
    let depth = text_input(SEARCH_DEPTH_PLACEHOLDER, &state.search_depth_text)
        .style(crate::view::input::style(palette))
        .on_input(Message::SettingsSearchDepthTyped)
        .on_submit(Message::SettingsSearchDepthApplied)
        .size(13.0)
        .width(Length::Fixed(COMPARE_LIMIT_FIELD_WIDTH))
        .font(Font::DEFAULT);
    let results = text_input(SEARCH_RESULTS_PLACEHOLDER, &state.search_results_text)
        .style(crate::view::input::style(palette))
        .on_input(Message::SettingsSearchResultsTyped)
        .on_submit(Message::SettingsSearchResultsApplied)
        .size(13.0)
        .width(Length::Fixed(COMPARE_LIMIT_FIELD_WIDTH))
        .font(Font::DEFAULT);
    column![
        section(&tr("Search"), palette),
        preview,
        wrapped_label(search_depth_label(), palette.fg),
        row![depth, text_button(&tr("Apply"), Some(Message::SettingsSearchDepthApplied), Look::Normal, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
        wrapped_label(search_results_label(), palette.fg),
        row![results, text_button(&tr("Apply"), Some(Message::SettingsSearchResultsApplied), Look::Normal, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
        wrapped_label(search_limits_help(), palette.fg_dim),
        restart_line(palette),
    ]
    .spacing(8)
    .into()
}

fn terminal_section<'a>(app: &'a App, state: &'a SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let input = text_input(&terminal_placeholder(), &state.terminal_text)
        .style(crate::view::input::style(palette))
        .id(App::settings_terminal_id())
        .on_input(Message::SettingsTerminalTyped)
        .on_submit(Message::SettingsTerminalApplied)
        .size(13.0)
        .font(Font::DEFAULT);
    let embedded = checkbox(app.gui_settings().embedded_terminal)
        .label(tr("Open terminals inside the window"))
        .text_size(13.0)
        .on_toggle(Message::SettingsEmbeddedTerminalToggled)
        .style(crate::view::checkbox::style(palette));
    column![
        section(&tr("Terminal"), palette),
        embedded,
        row![input, text_button(&tr("Apply"), Some(Message::SettingsTerminalApplied), Look::Normal, palette)]
            .spacing(8)
            .align_y(Alignment::Center),
        wrapped_label(terminal_help(), palette.fg_dim),
    ]
    .spacing(8)
    .into()
}

fn updates_section<'a>(app: &'a App) -> Element<'a, Message> {
    let palette = app.palette();
    column![
        section(&tr("Updates"), palette),
        label(tr("Check for new versions:"), palette.fg),
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
            let mut shortcuts = column![section(&tr("Keyboard shortcuts"), palette)].spacing(2);
            for action in Action::all() {
                shortcuts = shortcuts.push(shortcut_row(app, state, action));
            }
            shortcuts.into()
        }
        SettingsSection::Appearance => appearance_section(app),
        SettingsSection::Editing => editing_section(app, state),
        SettingsSection::Terminal => terminal_section(app, state),
        SettingsSection::Search => search_section(app, state),
        SettingsSection::Language => language_section(app),
        SettingsSection::Startup => startup_section(app),
        SettingsSection::Connections => connection_section(app),
        SettingsSection::Updates => updates_section(app),
        SettingsSection::Refresh => refresh_section(app, state),
        SettingsSection::Logs => logs_section(app, state),
    }
}

fn sidebar<'a>(app: &App, state: &SettingsState) -> Element<'a, Message> {
    let palette = app.palette();
    let mut entries = column![].spacing(2);
    for category in SettingsCategory::ALL {
        let selected = category == state.category;
        entries =
            entries.push(nav_item(&category.title(), selected, Message::SettingsCategoryChosen(category), palette));
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
        bold_label(tr("Settings"), palette.fg_bright),
        hairline_across(palette),
        row![sidebar(app, state), hairline_down(palette), list].spacing(8),
        hairline_across(palette),
        row![
            message_line(state, palette),
            text_button(&tr("Close"), Some(Message::SettingsClosed), Look::Primary, palette)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(MD)
    .into()
}

#[cfg(test)]
mod tests;
