mod conflict;
mod edit_diff;
mod properties;
mod sync_options;

use iced::widget::{checkbox, column, container, row, stack, text, text_input};
use iced::{Border, Element, Font, Length, alignment};
use porthmos_core::i18n::label as tr;

use super::button::{Look, text_button};
use super::{bold_label, label, wrapped_label};
use crate::app::{App, Message};
use crate::dialogs::{Dialog, Typed};
use crate::editing::waiting_notice;
use crate::modal::{DiffPane, Modal, delete_lines, edit_question_title, quit_text};
use crate::theme::Palette;
use crate::view::text_fit::fit;
use porthmos_core::edit::{EditChoice, EditQuestionKind, printable};
use porthmos_core::permissions::{mode_text, symbolic};

const DIALOG_WIDTH: f32 = 440.0;
const CONFLICT_WIDTH: f32 = 720.0;
const DIFF_WIDTH: f32 = 860.0;
const NAME_CELLS: usize = 60;

pub(super) fn button<'a>(title: &str, message: Message, palette: &Palette) -> Element<'a, Message> {
    text_button(title, Some(message), Look::Normal, palette)
}

fn primary_button<'a>(title: &str, message: Message, palette: &Palette) -> Element<'a, Message> {
    text_button(title, Some(message), Look::Primary, palette)
}

pub(super) fn body_text<'a>(content: String, palette: &Palette) -> Element<'a, Message> {
    text(content).size(13.0).font(Font::DEFAULT).color(palette.fg).into()
}

fn name_input<'a>(value: &str, palette: &Palette) -> Element<'a, Message> {
    text_input(&tr("Name"), value)
        .style(crate::view::input::style(palette))
        .id(App::dialog_input_id())
        .on_input(Message::ModalTyped)
        .on_submit(Message::ModalAccepted)
        .size(13.0)
        .font(Font::DEFAULT)
        .into()
}

fn name_prompt<'a>(value: &str, error: &Option<String>, palette: &Palette) -> Element<'a, Message> {
    let mut content = column![name_input(value, palette)].spacing(6);
    if let Some(error) = error {
        content = content.push(label(error.clone(), palette.err));
    }
    content.into()
}

const PERMISSION_CLASSES: [(&str, u32); 3] = [("User", 0o100), ("Group", 0o010), ("Other", 0o001)];
const PERMISSION_KINDS: [(&str, u32); 3] = [("Read", 4), ("Write", 2), ("Execute", 1)];
const PERMISSION_SPECIALS: [(&str, u32); 3] = [("Set user ID", 0o4000), ("Set group ID", 0o2000), ("Sticky", 0o1000)];
const PERMISSION_LABEL_WIDTH: f32 = 60.0;
const PERMISSION_CELL_WIDTH: f32 = 80.0;

fn permissions_title(entries: &[porthmos_core::Entry]) -> String {
    match entries {
        [only] => porthmos_core::i18n::text_with(
            "permissions.dialog_one",
            &[("name", &fit(&printable(&only.name), NAME_CELLS))],
        ),
        many => porthmos_core::i18n::text_with("permissions.dialog_many", &[("count", &many.len().to_string())]),
    }
}

fn permissions_body<'a>(mode: u32, recursive: bool, has_folder: bool, palette: &Palette) -> Element<'a, Message> {
    let grid = permissions_grid(mode, palette);
    if !has_folder {
        return grid;
    }
    let inside = checkbox(recursive)
        .label(tr("Apply to everything inside folders"))
        .on_toggle(Message::PermissionsRecursiveToggled)
        .style(crate::view::checkbox::style(palette));
    column![grid, inside].spacing(12).into()
}

fn permissions_grid<'a>(mode: u32, palette: &Palette) -> Element<'a, Message> {
    let cell = |content: Element<'a, Message>| container(content).width(Length::Fixed(PERMISSION_CELL_WIDTH));
    let mut header = row![label(String::new(), palette.fg_dim).width(Length::Fixed(PERMISSION_LABEL_WIDTH))].spacing(8);
    for (kind, _) in PERMISSION_KINDS {
        header = header.push(cell(label(tr(kind), palette.fg_dim).into()));
    }
    let mut rows = column![header].spacing(10);
    for (class, unit) in PERMISSION_CLASSES {
        let mut line = row![label(tr(class), palette.fg_dim).width(Length::Fixed(PERMISSION_LABEL_WIDTH))]
            .spacing(8)
            .align_y(alignment::Vertical::Center);
        for (_, weight) in PERMISSION_KINDS {
            let bit = unit * weight;
            line = line.push(cell(
                checkbox(mode & bit != 0)
                    .on_toggle(move |on| Message::PermissionToggled(bit, on))
                    .style(crate::view::checkbox::style(palette))
                    .into(),
            ));
        }
        rows = rows.push(line);
    }
    let mut specials = column![].spacing(6);
    for (name, bit) in PERMISSION_SPECIALS {
        specials = specials.push(
            checkbox(mode & bit != 0)
                .label(tr(name))
                .on_toggle(move |on| Message::PermissionToggled(bit, on))
                .style(crate::view::checkbox::style(palette)),
        );
    }
    column![rows, specials, label(format!("{}   {}", mode_text(mode), symbolic(mode)), palette.fg_dim)]
        .spacing(12)
        .into()
}

fn buttons<'a>(accept: &str, decline: &str, palette: &Palette) -> Element<'a, Message> {
    row![primary_button(accept, Message::ModalAccepted, palette), button(decline, Message::ModalDeclined, palette)]
        .spacing(8)
        .into()
}

fn cautious_buttons<'a>(accept: &str, decline: &str, palette: &Palette) -> Element<'a, Message> {
    row![button(accept, Message::ModalAccepted, palette), primary_button(decline, Message::ModalDeclined, palette)]
        .spacing(8)
        .into()
}

fn conflict_explanation() -> String {
    porthmos_core::i18n::text("edit.conflict_explanation")
}

fn question_width(diff: &DiffPane) -> f32 {
    match diff {
        DiffPane::Ready(_) => DIFF_WIDTH,
        DiffPane::Idle | DiffPane::Comparing | DiffPane::Failed(_) => DIALOG_WIDTH,
    }
}

fn conflict_content<'a>(diff: &DiffPane, palette: &Palette) -> Element<'a, Message> {
    match diff {
        DiffPane::Ready(outcome) => edit_diff::body(outcome, palette),
        DiffPane::Failed(message) => column![
            wrapped_label(edit_diff::failed_text(message), palette.err),
            body_text(conflict_explanation(), palette),
        ]
        .spacing(8)
        .into(),
        DiffPane::Idle | DiffPane::Comparing => body_text(conflict_explanation(), palette),
    }
}

fn conflict_buttons<'a>(diff: &DiffPane, palette: &Palette) -> Element<'a, Message> {
    let overwrite = button(&tr("Overwrite"), Message::EditAnswered(EditChoice::Upload), palette);
    let keep = primary_button(&tr("Keep mine as a copy"), Message::EditAnswered(EditChoice::KeepCopy), palette);
    let cancel = button(&tr("Cancel"), Message::EditAnswered(EditChoice::Cancel), palette);
    match diff {
        DiffPane::Ready(_) => {
            row![overwrite, keep, cancel, button(&tr("Back"), Message::EditDiffBack, palette)].spacing(8).into()
        }
        DiffPane::Comparing => {
            row![text_button(&tr("Comparing\u{2026}"), None, Look::Normal, palette), overwrite, keep, cancel]
                .spacing(8)
                .into()
        }
        DiffPane::Idle | DiffPane::Failed(_) => {
            row![button(&tr("Show differences"), Message::EditShowDiff, palette), overwrite, keep, cancel]
                .spacing(8)
                .into()
        }
    }
}

pub fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let palette = app.palette();
    let (title, content, buttons) = match app.modal()? {
        Modal::Question(Dialog::Confirm { title, body, .. }) => {
            (title.clone(), body_text(body.clone(), palette), cautious_buttons(&tr("Yes"), &tr("No"), palette))
        }
        Modal::Question(Dialog::Password { title, prompt, text: typed, save, .. }) => {
            let input = text_input("", typed.as_str())
                .style(crate::view::input::style(palette))
                .id(App::dialog_input_id())
                .secure(true)
                .on_input(|value| Message::DialogTyped(Typed::new(value)))
                .on_submit(Message::ModalAccepted)
                .size(13.0)
                .font(Font::DEFAULT);
            let mut content = column![body_text(prompt.clone(), palette), input].spacing(8);
            if let Some(save) = save {
                content = content.push(
                    checkbox(*save)
                        .label(tr("Save in keyring"))
                        .text_size(13.0)
                        .on_toggle(Message::PasswordSaveToggled)
                        .style(crate::view::checkbox::style(palette)),
                );
            }
            if let Some(hint) = app.keyring_hint() {
                content = content.push(wrapped_label(hint, palette.fg_dim));
            }
            (title.clone(), content.into(), buttons(&tr("OK"), &tr("Cancel"), palette))
        }
        Modal::NewFolder { text, error, .. } => {
            (tr("New folder"), name_prompt(text, error, palette), buttons(&tr("Create"), &tr("Cancel"), palette))
        }
        Modal::BookmarkName { text, error, .. } => {
            (tr("Bookmark this folder"), name_prompt(text, error, palette), buttons(&tr("Add"), &tr("Cancel"), palette))
        }
        Modal::Info { title, body } => (
            title.clone(),
            body_text(body.clone(), palette),
            primary_button(&tr("Close"), Message::ModalAccepted, palette),
        ),
        Modal::ConfirmClearHistory => (
            tr("Clear history"),
            body_text(tr("Clear the whole transfer history?"), palette),
            cautious_buttons(&tr("Clear"), &tr("Cancel"), palette),
        ),
        Modal::ConfirmPaste { text, .. } => (
            tr("Paste several lines?"),
            body_text(
                porthmos_core::i18n::text_with(
                    "gui.terminal.paste_question",
                    &[
                        ("count", &crate::drawer::paste::paste_line_count(text).to_string()),
                        ("preview", &crate::drawer::paste::paste_preview(text)),
                    ],
                ),
                palette,
            ),
            buttons(&tr("Paste"), &tr("Cancel"), palette),
        ),
        Modal::OpenLink { target } => (
            tr("Open link?"),
            body_text(porthmos_core::i18n::text_with("gui.terminal.link_question", &[("target", target)]), palette),
            cautious_buttons(&tr("Open"), &tr("Cancel"), palette),
        ),
        Modal::Permissions { mode, entries, recursive, .. } => (
            permissions_title(entries),
            permissions_body(*mode, *recursive, entries.iter().any(|entry| entry.is_dir), palette),
            buttons(&tr("Apply"), &tr("Cancel"), palette),
        ),
        Modal::Properties { pane, .. } => {
            (pane.title.clone(), properties::body(pane, palette), properties::buttons(pane, palette))
        }
        Modal::Rename { text, error, .. } => {
            (tr("Rename"), name_prompt(text, error, palette), buttons(&tr("Rename"), &tr("Cancel"), palette))
        }
        Modal::Conflict { view, apply_to_rest } => (
            conflict::title(view).to_string(),
            conflict::body(view, *apply_to_rest, palette),
            conflict::buttons(view, palette),
        ),
        Modal::EditQuestion { name, kind, diff, .. } => {
            let title = edit_question_title(name, *kind);
            match kind {
                EditQuestionKind::Upload => (
                    title,
                    body_text(tr("The file was changed. Upload it back to the server?"), palette),
                    row![
                        primary_button(&tr("Upload"), Message::EditAnswered(EditChoice::Upload), palette),
                        button(&tr("Discard"), Message::EditAnswered(EditChoice::Cancel), palette),
                    ]
                    .spacing(8)
                    .into(),
                ),
                EditQuestionKind::Conflict => (title, conflict_content(diff, palette), conflict_buttons(diff, palette)),
            }
        }
        Modal::EditWaiting { name, .. } => (
            tr("Editing"),
            body_text(waiting_notice(name), palette),
            primary_button(&tr("Done"), Message::EditDoneEditing, palette),
        ),
        Modal::ConfirmQuit { transfers, edits } => (
            tr("Quit Porthmos"),
            body_text(quit_text(*transfers, *edits), palette),
            cautious_buttons(&tr("Quit"), &tr("Cancel"), palette),
        ),
        Modal::SyncOptions { name, offer_both, direction, by, subfolders, mirror, error, .. } => (
            porthmos_core::i18n::text_with("sync.form_title", &[("name", &fit(&printable(name), NAME_CELLS))]),
            sync_options::body(*direction, *by, *subfolders, *mirror, *offer_both, error, palette),
            buttons(&tr("Start"), &tr("Cancel"), palette),
        ),
        Modal::ConfirmSyncDelete { message, .. } => {
            (tr("Delete"), body_text(message.clone(), palette), cautious_buttons(&tr("Delete"), &tr("Cancel"), palette))
        }
        Modal::ConfirmDelete { entries, place, count, .. } => {
            let lines = delete_lines(entries, place, count.as_ref()).join("\n");
            (tr("Delete"), body_text(lines, palette), cautious_buttons(&tr("Delete"), &tr("Cancel"), palette))
        }
    };
    let width = match app.modal() {
        Some(Modal::Conflict { .. }) => CONFLICT_WIDTH,
        Some(Modal::EditQuestion { diff, .. }) => question_width(diff),
        Some(Modal::Properties { .. }) => properties::WIDTH,
        _ => DIALOG_WIDTH,
    };
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let surface = palette.surface;
    let body = column![bold_label(title, palette.fg_bright), content, buttons].spacing(12);
    let boxed = container(body).padding(16).width(Length::Fill).max_width(width).style(move |_| container::Style {
        background: Some(iced::Background::Color(surface)),
        border,
        ..container::Style::default()
    });
    let backdrop = crate::view::scrim(palette);
    let placed = container(boxed)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);
    Some(stack![backdrop, placed].into())
}

#[cfg(test)]
mod tests;
