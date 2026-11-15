use iced::widget::{checkbox, column, container, mouse_area, pick_list, row, scrollable, space, stack, text_input};
use iced::{Alignment, Border, Element, Font, Length, Size, alignment};
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;

use super::button::{Look, compact_button, text_button};
use super::{bold_label, label, wrapped_label};
use crate::app::{App, Message};
use crate::connections::form::{Choice, FieldKind, FieldView, FormState};
use crate::connections::trial::{TestPrompt, TestState, describe};
use crate::connections::{LabelsState, MissingChoice, Screen};
use crate::dialogs::Typed;
use crate::theme::Palette;

pub const FORM_WIDTH: f32 = 560.0;
pub const FORM_HEIGHT: f32 = 520.0;
const SMALL_WIDTH: f32 = 440.0;
const MARGIN: f32 = 16.0;
const LABEL_WIDTH: f32 = 130.0;

pub fn dialog_size(window: Size, width: f32, height: f32) -> Size {
    Size::new(width.min((window.width - 2.0 * MARGIN).max(0.0)), height.min((window.height - 2.0 * MARGIN).max(0.0)))
}

pub fn field_label(field: &FieldView) -> String {
    let shown = tr(&field.label);
    if field.required { i18n::text_with("gui.form.required", &[("label", &shown)]) } else { shown }
}

pub fn missing_title(name: &str, shadowed: bool) -> String {
    if shadowed {
        i18n::text_with("missing.item_hidden", &[("name", name)])
    } else {
        i18n::text_with("connection.no_longer_in_ssh_config", &[("name", name)])
    }
}

pub fn secret_placeholder(saved: bool, forgotten: bool) -> String {
    match (saved, forgotten) {
        (true, true) => tr("Will be removed when you save"),
        (true, false) => tr("Saved. Type to replace"),
        (false, _) => String::new(),
    }
}

fn input<'a>(placeholder: &str, value: &str, palette: &Palette) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value).size(13.0).font(Font::DEFAULT).style(crate::view::input::style(palette))
}

fn control<'a>(state: &'a FormState, field: &FieldView, palette: &Palette) -> Element<'a, Message> {
    let key = field.key.clone();
    match &field.kind {
        FieldKind::Text(value) => input("", value, palette)
            .id(App::form_input_id(&key))
            .on_input(move |typed| Message::FormTextTyped(key.clone(), typed))
            .into(),
        FieldKind::Secret { saved, forgotten, .. } => {
            let secret_key = key.clone();
            let field_input = input(&secret_placeholder(*saved, *forgotten), state.secret_text(&key), palette)
                .id(App::form_input_id(&key))
                .secure(true)
                .on_input(move |typed| Message::FormSecretTyped(secret_key.clone(), Typed::new(typed)));
            let mut line = row![field_input].spacing(8).align_y(Alignment::Center);
            if *saved && !*forgotten {
                line = line.push(compact_button(&tr("Forget"), Some(Message::FormSecretForgotten(key)), palette));
            } else if *forgotten {
                line = line.push(compact_button(&tr("Keep saved"), Some(Message::FormSecretRestored(key)), palette));
            }
            line.into()
        }
        FieldKind::Choice { choices, selected } => {
            pick_list(choices.clone(), Some(selected.clone()), move |choice: Choice| {
                Message::FormChoiceChanged(key.clone(), choice.value)
            })
            .text_size(13.0)
            .padding([6.0, 8.0])
            .style(crate::view::dropdown::style(palette))
            .menu_style(crate::view::dropdown::menu_style(palette))
            .width(Length::Fill)
            .into()
        }
        FieldKind::Toggle(on) => checkbox(*on)
            .on_toggle(move |value| Message::FormToggled(key.clone(), value))
            .style(crate::view::checkbox::style(palette))
            .into(),
    }
}

fn form_body<'a>(app: &'a App, state: &'a FormState) -> (Element<'a, Message>, Element<'a, Message>) {
    let palette = app.palette();
    let mut fields = column![].spacing(8);
    for field in state.fields() {
        let line = row![
            label(field_label(&field), palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)),
            container(control(state, &field, palette)).width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(8);
        fields = fields.push(line);
    }
    let mut body =
        column![scrollable(fields).style(crate::view::scroll::style(palette)).height(Length::Shrink)].spacing(8);
    if let Some(error) = &state.error {
        body = body.push(wrapped_label(error.clone(), palette.err));
    }
    if let Some(hint) = app.keyring_hint() {
        body = body.push(wrapped_label(hint, palette.fg_dim));
    }
    if let Some(line) = test_line(app, state) {
        body = body.push(line);
    }
    let test_button = match state.test {
        Some(TestState::Running) => {
            text_button(&tr("Cancel test"), Some(Message::FormTestCancelled), Look::Normal, palette)
        }
        _ => text_button(
            &tr("Test connection"),
            state.check().is_ok().then_some(Message::FormTestPressed),
            Look::Normal,
            palette,
        ),
    };
    let footer = row![
        test_button,
        space().width(Length::Fill),
        text_button(&tr("Cancel"), Some(Message::ScreenCancelled), Look::Normal, palette),
        text_button(&tr("Save"), Some(Message::FormSavePressed), Look::Primary, palette),
    ]
    .spacing(8)
    .into();
    (body.into(), footer)
}

fn test_line<'a>(app: &App, state: &FormState) -> Option<Element<'a, Message>> {
    let palette = app.palette();
    match &state.test {
        None => None,
        Some(TestState::Running) => Some(label(tr("Testing\u{2026}"), palette.fg_muted).into()),
        Some(TestState::Finished(outcome)) => {
            let line = describe(outcome)?;
            let color = if line.good { palette.ok } else { palette.err };
            Some(row![label("\u{25cf}", color), wrapped_label(line.text, palette.fg)].spacing(8).into())
        }
    }
}

fn prompt_dialog<'a>(app: &App, prompt: &TestPrompt) -> Element<'a, Message> {
    let palette = app.palette();
    let field = input("", prompt.text.as_str(), palette)
        .id(App::test_password_id())
        .secure(true)
        .on_input(|typed| Message::TestPasswordTyped(Typed::new(typed)))
        .on_submit(Message::TestPasswordSubmitted);
    let buttons = row![
        space().width(Length::Fill),
        text_button(&tr("Cancel"), Some(Message::TestPasswordCancelled), Look::Normal, palette),
        text_button(&tr("OK"), Some(Message::TestPasswordSubmitted), Look::Primary, palette),
    ]
    .spacing(8);
    let content =
        column![bold_label(prompt.secret, palette.fg_bright), label(prompt.heading(), palette.fg), field, buttons]
            .spacing(12);
    let surface = palette.surface;
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let boxed = container(content).padding(16).width(Length::Fixed(SMALL_WIDTH)).style(move |_| container::Style {
        background: Some(iced::Background::Color(surface)),
        border,
        ..container::Style::default()
    });
    let blocked = mouse_area(boxed).on_press(Message::ManagerIgnored);
    let backdrop = mouse_area(crate::view::scrim(palette)).on_press(Message::ManagerIgnored);
    let placed = container(blocked)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);
    stack![backdrop, placed].into()
}

fn labels_body<'a>(app: &'a App, labels: &'a LabelsState) -> (Element<'a, Message>, Element<'a, Message>) {
    let palette = app.palette();
    let line = |name: &str, value: &str, message: fn(String) -> Message, id: &str| -> Element<'a, Message> {
        row![
            label(name, palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)),
            input("", value, palette).id(App::form_input_id(id)).on_input(message).width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(8)
        .into()
    };
    let mut body = column![
        label(tr("Labels are kept by Porthmos. Your ~/.ssh/config is not changed."), palette.fg_muted),
        line(&tr("Group"), &labels.group, Message::LabelsGroupTyped, "group"),
        line(&tr("Tags"), &labels.tags, Message::LabelsTagsTyped, "tags"),
    ]
    .spacing(8);
    if labels.saved_password {
        body = body.push(
            checkbox(labels.forget_password)
                .label(tr("Forget the saved password"))
                .text_size(13.0)
                .on_toggle(Message::LabelsForgetToggled)
                .style(crate::view::checkbox::style(palette)),
        );
    }
    if let Some(error) = &labels.error {
        body = body.push(label(error.clone(), palette.err));
    }
    let footer = row![
        space().width(Length::Fill),
        text_button(&tr("Cancel"), Some(Message::ScreenCancelled), Look::Normal, palette),
        text_button(&tr("Save"), Some(Message::LabelsSavePressed), Look::Primary, palette),
    ]
    .spacing(8)
    .into();
    (body.into(), footer)
}

fn delete_body<'a>(app: &App, name: &str) -> (Element<'a, Message>, Element<'a, Message>) {
    let palette = app.palette();
    let body = label(i18n::text_with("gui.form.delete_confirm", &[("name", name)]), palette.fg).into();
    let footer = row![
        space().width(Length::Fill),
        text_button(&tr("Delete"), Some(Message::DeleteConfirmed), Look::Normal, palette),
        text_button(&tr("Cancel"), Some(Message::ScreenCancelled), Look::Primary, palette),
    ]
    .spacing(8)
    .into();
    (body, footer)
}

fn missing_body<'a>(app: &App, name: &str, shadowed: bool) -> (Element<'a, Message>, Element<'a, Message>) {
    let palette = app.palette();
    let candidates = app.manager().unlabelled_hosts();
    let mut body = column![label(missing_title(name, shadowed), palette.fg)].spacing(8);
    if candidates.is_empty() {
        body = body.push(label(tr("No unlabelled ssh hosts to move the labels to"), palette.fg_dim));
    }
    for candidate in candidates {
        let title = i18n::text_with("gui.form.move_labels_to", &[("name", &candidate)]);
        body = body.push(text_button(
            &title,
            Some(Message::MissingHostChosen(MissingChoice::Move(candidate))),
            Look::Normal,
            palette,
        ));
    }
    body = body.push(text_button(
        &tr("Forget labels"),
        Some(Message::MissingHostChosen(MissingChoice::Forget)),
        Look::Normal,
        palette,
    ));
    let footer = row![
        space().width(Length::Fill),
        text_button(&tr("Cancel"), Some(Message::ScreenCancelled), Look::Normal, palette)
    ]
    .into();
    (body.into(), footer)
}

pub fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let palette = app.palette();
    let (title, (body, footer), width, height) = match app.screen()? {
        Screen::List => return None,
        Screen::Form(state) => (state.title().to_string(), form_body(app, state), FORM_WIDTH, FORM_HEIGHT),
        Screen::Labels(labels) => (
            i18n::text_with("gui.form.labels_for", &[("name", &labels.name)]),
            labels_body(app, labels),
            SMALL_WIDTH,
            260.0,
        ),
        Screen::ConfirmDelete { name } => (tr("Delete connection"), delete_body(app, name), SMALL_WIDTH, 150.0),
        Screen::ImportSource(state) => (
            tr("Import connections"),
            (
                super::import_source::body(state.source, &state.path, &state.error, state.waiting, palette),
                super::import_source::buttons(state.waiting, palette),
            ),
            SMALL_WIDTH.max(super::import_source::WIDTH),
            super::import_source::HEIGHT,
        ),
        Screen::ImportReview(view) => (
            i18n::text_with("gui.import.from", &[("source", view.source().label())]),
            (super::import_review::body(view, palette), super::import_review::buttons(view, palette)),
            super::import_review::WIDTH,
            super::import_review::HEIGHT,
        ),
        Screen::MissingHost { name, shadowed } => {
            (tr("Labelled ssh host"), missing_body(app, name, *shadowed), SMALL_WIDTH, 300.0)
        }
    };
    let size = dialog_size(app.connections_window_size(), width, height);
    let surface = palette.surface;
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let content = column![
        bold_label(title, palette.fg_bright),
        crate::view::hairline_across(palette),
        container(body).height(Length::Shrink),
        crate::view::hairline_across(palette),
        footer
    ]
    .spacing(12);
    let boxed =
        container(content).padding(16).width(Length::Fixed(size.width)).max_height(size.height).style(move |_| {
            container::Style {
                background: Some(iced::Background::Color(surface)),
                border,
                ..container::Style::default()
            }
        });
    let blocked = mouse_area(boxed).on_press(Message::ManagerIgnored);
    let backdrop = mouse_area(crate::view::scrim(palette)).on_press(Message::ManagerIgnored);
    let placed = container(blocked)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);
    let dialog: Element<'_, Message> = stack![backdrop, placed].into();
    match app.screen() {
        Some(Screen::Form(state)) if state.prompt.is_some() => {
            let prompt = state.prompt.as_ref()?;
            Some(stack![dialog, prompt_dialog(app, prompt)].into())
        }
        _ => Some(dialog),
    }
}

#[cfg(test)]
mod tests;
