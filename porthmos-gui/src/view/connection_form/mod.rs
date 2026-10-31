use iced::widget::{checkbox, column, container, mouse_area, pick_list, row, scrollable, space, stack, text_input};
use iced::{Alignment, Border, Element, Font, Length, Size, alignment};

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
    if field.required { format!("{} *", field.label) } else { field.label.clone() }
}

pub fn missing_title(name: &str, shadowed: bool) -> String {
    if shadowed {
        format!("{name} is hidden by a saved connection")
    } else {
        format!("{name} is no longer in ~/.ssh/config")
    }
}

pub fn secret_placeholder(saved: bool, forgotten: bool) -> &'static str {
    match (saved, forgotten) {
        (true, true) => "Will be removed when you save",
        (true, false) => "Saved. Type to replace",
        (false, _) => "",
    }
}

fn input<'a>(placeholder: &str, value: &str) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value).size(13.0).font(Font::DEFAULT)
}

fn control<'a>(state: &'a FormState, field: &FieldView, palette: &Palette) -> Element<'a, Message> {
    let key = field.key.clone();
    match &field.kind {
        FieldKind::Text(value) => input("", value)
            .id(App::form_input_id(&key))
            .on_input(move |typed| Message::FormTextTyped(key.clone(), typed))
            .into(),
        FieldKind::Secret { saved, forgotten, .. } => {
            let secret_key = key.clone();
            let field_input = input(secret_placeholder(*saved, *forgotten), state.secret_text(&key))
                .id(App::form_input_id(&key))
                .secure(true)
                .on_input(move |typed| Message::FormSecretTyped(secret_key.clone(), Typed::new(typed)));
            let mut line = row![field_input].spacing(8).align_y(Alignment::Center);
            if *saved && !*forgotten {
                line = line.push(compact_button("Forget", Some(Message::FormSecretForgotten(key)), palette));
            } else if *forgotten {
                line = line.push(compact_button("Keep saved", Some(Message::FormSecretRestored(key)), palette));
            }
            line.into()
        }
        FieldKind::Choice { choices, selected } => {
            pick_list(choices.clone(), Some(selected.clone()), move |choice: Choice| {
                Message::FormChoiceChanged(key.clone(), choice.value)
            })
            .text_size(13.0)
            .width(Length::Fill)
            .into()
        }
        FieldKind::Toggle(on) => checkbox(*on).on_toggle(move |value| Message::FormToggled(key.clone(), value)).into(),
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
    let mut body = column![scrollable(fields).height(Length::Fill)].spacing(8);
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
        Some(TestState::Running) => text_button("Cancel test", Some(Message::FormTestCancelled), Look::Normal, palette),
        _ => text_button(
            "Test connection",
            state.check().is_ok().then_some(Message::FormTestPressed),
            Look::Normal,
            palette,
        ),
    };
    let footer = row![
        test_button,
        space().width(Length::Fill),
        text_button("Cancel", Some(Message::ScreenCancelled), Look::Normal, palette),
        text_button("Save", Some(Message::FormSavePressed), Look::Primary, palette),
    ]
    .spacing(8)
    .into();
    (body.into(), footer)
}

fn test_line<'a>(app: &App, state: &FormState) -> Option<Element<'a, Message>> {
    let palette = app.palette();
    match &state.test {
        None => None,
        Some(TestState::Running) => Some(label("Testing\u{2026}", palette.fg_muted).into()),
        Some(TestState::Finished(outcome)) => {
            let line = describe(outcome)?;
            let color = if line.good { palette.ok } else { palette.err };
            Some(row![label("\u{25cf}", color), wrapped_label(line.text, palette.fg)].spacing(8).into())
        }
    }
}

fn prompt_dialog<'a>(app: &App, prompt: &TestPrompt) -> Element<'a, Message> {
    let palette = app.palette();
    let field = input("", prompt.text.as_str())
        .id(App::test_password_id())
        .secure(true)
        .on_input(|typed| Message::TestPasswordTyped(Typed::new(typed)))
        .on_submit(Message::TestPasswordSubmitted);
    let buttons = row![
        space().width(Length::Fill),
        text_button("Cancel", Some(Message::TestPasswordCancelled), Look::Normal, palette),
        text_button("OK", Some(Message::TestPasswordSubmitted), Look::Primary, palette),
    ]
    .spacing(8);
    let content = column![
        bold_label("Password", palette.fg_bright),
        label(format!("Password for {}@{}", prompt.username, prompt.name), palette.fg),
        field,
        buttons
    ]
    .spacing(12);
    let surface = palette.surface;
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let boxed = container(content).padding(16).width(Length::Fixed(SMALL_WIDTH)).style(move |_| container::Style {
        background: Some(iced::Background::Color(surface)),
        border,
        ..container::Style::default()
    });
    let blocked = mouse_area(boxed).on_press(Message::ManagerIgnored);
    let backdrop =
        mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::ManagerIgnored);
    let placed = container(blocked)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);
    stack![backdrop, placed].into()
}

fn labels_body<'a>(app: &'a App, labels: &'a LabelsState) -> (Element<'a, Message>, Element<'a, Message>) {
    let palette = app.palette();
    let line = |name: &'static str, value: &str, message: fn(String) -> Message, id: &str| -> Element<'a, Message> {
        row![
            label(name, palette.fg_dim).width(Length::Fixed(LABEL_WIDTH)),
            input("", value).id(App::form_input_id(id)).on_input(message).width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(8)
        .into()
    };
    let mut body = column![
        label("Labels are kept by Porthmos. Your ~/.ssh/config is not changed.", palette.fg_muted),
        line("Group", &labels.group, Message::LabelsGroupTyped, "group"),
        line("Tags", &labels.tags, Message::LabelsTagsTyped, "tags"),
    ]
    .spacing(8);
    if labels.saved_password {
        body = body.push(
            checkbox(labels.forget_password).label("Forget the saved password").on_toggle(Message::LabelsForgetToggled),
        );
    }
    if let Some(error) = &labels.error {
        body = body.push(label(error.clone(), palette.err));
    }
    let footer = row![
        space().width(Length::Fill),
        text_button("Cancel", Some(Message::ScreenCancelled), Look::Normal, palette),
        text_button("Save", Some(Message::LabelsSavePressed), Look::Primary, palette),
    ]
    .spacing(8)
    .into();
    (body.into(), footer)
}

fn delete_body<'a>(app: &App, name: &str) -> (Element<'a, Message>, Element<'a, Message>) {
    let palette = app.palette();
    let body = label(format!("Delete connection \"{name}\"?"), palette.fg).into();
    let footer = row![
        space().width(Length::Fill),
        text_button("Delete", Some(Message::DeleteConfirmed), Look::Normal, palette),
        text_button("Cancel", Some(Message::ScreenCancelled), Look::Primary, palette),
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
        body = body.push(label("No unlabelled ssh hosts to move the labels to", palette.fg_dim));
    }
    for candidate in candidates {
        let title = format!("Move labels to {candidate}");
        body = body.push(text_button(
            &title,
            Some(Message::MissingHostChosen(MissingChoice::Move(candidate))),
            Look::Normal,
            palette,
        ));
    }
    body = body.push(text_button(
        "Forget labels",
        Some(Message::MissingHostChosen(MissingChoice::Forget)),
        Look::Normal,
        palette,
    ));
    let footer =
        row![space().width(Length::Fill), text_button("Cancel", Some(Message::ScreenCancelled), Look::Normal, palette)]
            .into();
    (body.into(), footer)
}

pub fn overlay(app: &App) -> Option<Element<'_, Message>> {
    let palette = app.palette();
    let (title, (body, footer), width, height) = match app.screen()? {
        Screen::List => return None,
        Screen::Form(state) => (state.title().to_string(), form_body(app, state), FORM_WIDTH, FORM_HEIGHT),
        Screen::Labels(labels) => (format!("Labels for {}", labels.name), labels_body(app, labels), SMALL_WIDTH, 260.0),
        Screen::ConfirmDelete { name } => ("Delete connection".to_string(), delete_body(app, name), SMALL_WIDTH, 150.0),
        Screen::MissingHost { name, shadowed } => {
            ("Labelled ssh host".to_string(), missing_body(app, name, *shadowed), SMALL_WIDTH, 300.0)
        }
    };
    let size = dialog_size(app.window_size(), width, height);
    let surface = palette.surface;
    let border = Border { color: palette.line_strong, width: 1.0, radius: 6.0.into() };
    let content =
        column![bold_label(title, palette.fg_bright), container(body).height(Length::Fill), footer].spacing(12);
    let boxed =
        container(content).padding(16).width(Length::Fixed(size.width)).height(Length::Fixed(size.height)).style(
            move |_| container::Style {
                background: Some(iced::Background::Color(surface)),
                border,
                ..container::Style::default()
            },
        );
    let blocked = mouse_area(boxed).on_press(Message::ManagerIgnored);
    let backdrop =
        mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::ManagerIgnored);
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
