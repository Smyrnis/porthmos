use iced::widget::{column, text_input};
use iced::{Element, Font};
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{ProcessSignal, edit::printable};

use super::super::{label, wrapped_label};
use super::NAME_CELLS;
use crate::app::{App, Message};
use crate::theme::Palette;
use crate::view::text_fit::fit;

pub fn signal_title(signal: ProcessSignal) -> String {
    match signal {
        ProcessSignal::Terminate => tr("Terminate process"),
        ProcessSignal::Kill => tr("Force kill process"),
    }
}

pub fn signal_accept(signal: ProcessSignal) -> String {
    match signal {
        ProcessSignal::Terminate => tr("Terminate"),
        ProcessSignal::Kill => tr("Force kill"),
    }
}

fn shown_name(name: &str) -> String {
    fit(&printable(name), NAME_CELLS).to_string()
}

pub fn signal_question(signal: ProcessSignal, name: &str, pid: u32) -> String {
    let key = match signal {
        ProcessSignal::Terminate => "gui.health.terminate_question",
        ProcessSignal::Kill => "gui.health.kill_question",
    };
    i18n::text_with(key, &[("name", &shown_name(name)), ("pid", &pid.to_string())])
}

pub fn renice_body<'a>(
    name: &str, pid: u32, value: &str, error: &Option<String>, palette: &Palette,
) -> Element<'a, Message> {
    let question =
        i18n::text_with("gui.health.renice_question", &[("name", &shown_name(name)), ("pid", &pid.to_string())]);
    let input = text_input(&tr("Priority"), value)
        .style(crate::view::input::style(palette))
        .id(App::dialog_input_id())
        .on_input(Message::HealthReniceChanged)
        .on_submit(Message::ModalAccepted)
        .size(13.0)
        .font(Font::DEFAULT);
    let mut content = column![wrapped_label(question, palette.fg), input].spacing(6);
    if let Some(error) = error {
        content = content.push(label(error.clone(), palette.err));
    }
    content.push(wrapped_label(i18n::text("gui.health.renice_note"), palette.fg_dim)).into()
}

#[cfg(test)]
mod tests;
