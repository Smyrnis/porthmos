use iced::Element;
use iced::widget::column;
use porthmos_core::edit::printable;
use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{Privilege, ServiceVerb};

use super::super::wrapped_label;
use super::NAME_CELLS;
use crate::app::Message;
use crate::theme::Palette;
use crate::view::spacing::SM;
use crate::view::text_fit::fit;

pub fn title(verb: ServiceVerb) -> String {
    match verb {
        ServiceVerb::Start => tr("Start service"),
        ServiceVerb::Stop => tr("Stop service"),
        ServiceVerb::Restart => tr("Restart service"),
        ServiceVerb::Enable => tr("Enable service"),
        ServiceVerb::Disable => tr("Disable service"),
    }
}

pub fn accept(verb: ServiceVerb) -> String {
    match verb {
        ServiceVerb::Start => tr("Start"),
        ServiceVerb::Stop => tr("Stop"),
        ServiceVerb::Restart => tr("Restart"),
        ServiceVerb::Enable => tr("Enable"),
        ServiceVerb::Disable => tr("Disable"),
    }
}

pub fn question(verb: ServiceVerb, unit: &str) -> String {
    let key = match verb {
        ServiceVerb::Start => "gui.services.start_question",
        ServiceVerb::Stop => "gui.services.stop_question",
        ServiceVerb::Restart => "gui.services.restart_question",
        ServiceVerb::Enable => "gui.services.enable_question",
        ServiceVerb::Disable => "gui.services.disable_question",
    };
    i18n::text_with(key, &[("name", &fit(&printable(unit), NAME_CELLS))])
}

pub fn sudo_note(privilege: Privilege) -> Option<String> {
    (privilege == Privilege::Sudo).then(|| i18n::text("gui.services.runs_with_sudo"))
}

pub fn body<'a>(unit: &str, verb: ServiceVerb, privilege: Privilege, palette: &Palette) -> Element<'a, Message> {
    let mut content = column![wrapped_label(question(verb, unit), palette.fg)].spacing(SM);
    if let Some(note) = sudo_note(privilege) {
        content = content.push(wrapped_label(note, palette.fg_dim));
    }
    content.into()
}

#[cfg(test)]
mod tests;
