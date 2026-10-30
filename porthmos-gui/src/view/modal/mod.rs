mod conflict;
mod sync_options;

use iced::widget::{checkbox, column, container, row, space, stack, text, text_input};
use iced::{Border, Element, Font, Length, alignment};

use super::button::{Look, text_button};
use super::{bold_label, label};
use crate::app::{App, Message};
use crate::dialogs::{Dialog, Typed};
use crate::editing::waiting_notice;
use crate::modal::{Modal, delete_summary, edit_question_title, quit_text};
use crate::theme::Palette;
use crate::view::text_fit::fit;
use porthmos_core::edit::{EditChoice, EditQuestionKind, printable};

const DIALOG_WIDTH: f32 = 440.0;
const CONFLICT_WIDTH: f32 = 720.0;
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

fn name_input<'a>(value: &str) -> Element<'a, Message> {
    text_input("Name", value)
        .id(App::dialog_input_id())
        .on_input(Message::ModalTyped)
        .on_submit(Message::ModalAccepted)
        .size(13.0)
        .font(Font::DEFAULT)
        .into()
}

fn name_prompt<'a>(value: &str, error: &Option<String>, palette: &Palette) -> Element<'a, Message> {
    let mut content = column![name_input(value)].spacing(6);
    if let Some(error) = error {
        content = content.push(label(error.clone(), palette.err));
    }
    content.into()
}

fn buttons<'a>(accept: &str, decline: &str, palette: &Palette) -> Element<'a, Message> {
    row![primary_button(accept, Message::ModalAccepted, palette), button(decline, Message::ModalDeclined, palette)]
        .spacing(8)
        .into()
}

pub fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let palette = app.palette();
    let (title, content, buttons) = match app.modal()? {
        Modal::Question(Dialog::Confirm { title, body, .. }) => {
            (title.clone(), body_text(body.clone(), palette), buttons("Yes", "No", palette))
        }
        Modal::Question(Dialog::Password { title, prompt, text: typed, save, .. }) => {
            let input = text_input("", typed.as_str())
                .id(App::dialog_input_id())
                .secure(true)
                .on_input(|value| Message::DialogTyped(Typed::new(value)))
                .on_submit(Message::ModalAccepted)
                .size(13.0)
                .font(Font::DEFAULT);
            let mut content = column![body_text(prompt.clone(), palette), input].spacing(8);
            if let Some(save) = save {
                content =
                    content.push(checkbox(*save).label("Save in keyring").on_toggle(Message::PasswordSaveToggled));
            }
            if let Some(hint) = app.keyring_hint() {
                content = content.push(label(hint, palette.fg_dim));
            }
            (title.clone(), content.into(), buttons("OK", "Cancel", palette))
        }
        Modal::NewFolder { text, error, .. } => {
            ("New folder".to_string(), name_prompt(text, error, palette), buttons("Create", "Cancel", palette))
        }
        Modal::BookmarkName { text, error, .. } => {
            ("Bookmark this folder".to_string(), name_prompt(text, error, palette), buttons("Add", "Cancel", palette))
        }
        Modal::Info { title, body } => {
            (title.clone(), body_text(body.clone(), palette), primary_button("Close", Message::ModalAccepted, palette))
        }
        Modal::ConfirmClearHistory => (
            "Clear history".to_string(),
            body_text("Clear the whole transfer history?".to_string(), palette),
            buttons("Clear", "Cancel", palette),
        ),
        Modal::Rename { text, error, .. } => {
            ("Rename".to_string(), name_prompt(text, error, palette), buttons("Rename", "Cancel", palette))
        }
        Modal::Conflict { view, apply_to_rest } => (
            "File already exists".to_string(),
            conflict::body(view, *apply_to_rest, palette),
            conflict::buttons(view, palette),
        ),
        Modal::EditQuestion { name, kind, .. } => {
            let title = edit_question_title(name, *kind);
            match kind {
                EditQuestionKind::Upload => (
                    title,
                    body_text("The file was changed. Upload it back to the server?".to_string(), palette),
                    row![
                        primary_button("Upload", Message::EditAnswered(EditChoice::Upload), palette),
                        button("Discard", Message::EditAnswered(EditChoice::Cancel), palette),
                    ]
                    .spacing(8)
                    .into(),
                ),
                EditQuestionKind::Conflict => (
                    title,
                    body_text(
                        "Overwrite the server's copy with yours, keep yours as a separate copy next to it, or cancel and keep the temporary file."
                            .to_string(),
                        palette,
                    ),
                    row![
                        button("Overwrite", Message::EditAnswered(EditChoice::Upload), palette),
                        primary_button("Keep mine as a copy", Message::EditAnswered(EditChoice::KeepCopy), palette),
                        button("Cancel", Message::EditAnswered(EditChoice::Cancel), palette),
                    ]
                    .spacing(8)
                    .into(),
                ),
            }
        }
        Modal::EditWaiting { name, .. } => (
            "Editing".to_string(),
            body_text(waiting_notice(name), palette),
            primary_button("Done", Message::EditDoneEditing, palette),
        ),
        Modal::ConfirmQuit { transfers, edits } => (
            "Quit Porthmos".to_string(),
            body_text(quit_text(*transfers, *edits), palette),
            buttons("Quit", "Cancel", palette),
        ),
        Modal::SyncOptions { name, offer_both, direction, by, subfolders, error, .. } => (
            format!("Synchronize with {}", fit(&printable(name), NAME_CELLS)),
            sync_options::body(*direction, *by, *subfolders, *offer_both, error, palette),
            buttons("Start", "Cancel", palette),
        ),
        Modal::ConfirmDelete { entries, .. } => {
            let lines = delete_summary(entries).join("\n");
            ("Delete".to_string(), body_text(lines, palette), buttons("Delete", "Cancel", palette))
        }
    };
    let width = if matches!(app.modal(), Some(Modal::Conflict { .. })) { CONFLICT_WIDTH } else { DIALOG_WIDTH };
    let border = Border { color: palette.line_strong, width: 1.0, ..Border::default() };
    let surface = palette.surface;
    let body = column![bold_label(title, palette.fg_bright), content, buttons].spacing(12);
    let boxed = container(body).padding(16).width(Length::Fixed(width)).style(move |_| container::Style {
        background: Some(iced::Background::Color(surface)),
        border,
        ..container::Style::default()
    });
    let backdrop = container(space()).width(Length::Fill).height(Length::Fill);
    let placed = container(boxed)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);
    Some(stack![backdrop, placed].into())
}
