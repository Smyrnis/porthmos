use porthmos_core::ProtocolInfo;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    theme::Theme,
    widgets::{
        dialog::{FieldKind, FormDialog},
        form_fields::render_field_stack,
        frame::{accent_underline, hairline, vertical_rule},
        history_view::printable,
        text_fit::truncate_with_ellipsis,
    },
};

const WIDE_LIST: u16 = 42;
const MEDIUM_LIST: u16 = 32;
const NAMES_LIST: u16 = 22;
const MIN_WIDTH_WITH_DESCRIPTIONS: u16 = 80;
const MIN_WIDTH_WITH_LIST: u16 = 60;
const WIDE_LIST_MIN_WIDTH: u16 = 100;
const GUTTER_AFTER_DIVIDER: u16 = 2;
const NOT_AVAILABLE: &str = "(not available)";

#[derive(Clone, Copy)]
struct ListMode {
    width: u16,
    descriptions: bool,
}

fn list_mode(width: u16) -> Option<ListMode> {
    if width >= WIDE_LIST_MIN_WIDTH {
        Some(ListMode { width: WIDE_LIST, descriptions: true })
    } else if width >= MIN_WIDTH_WITH_DESCRIPTIONS {
        Some(ListMode { width: MEDIUM_LIST, descriptions: true })
    } else if width >= MIN_WIDTH_WITH_LIST {
        Some(ListMode { width: NAMES_LIST, descriptions: false })
    } else {
        None
    }
}

fn protocol_choices(dialog: &FormDialog) -> (&[(String, String)], usize) {
    match dialog.fields.first().map(|field| &field.kind) {
        Some(FieldKind::Choice { choices, selected }) => (choices.as_slice(), *selected),
        _ => (&[], 0),
    }
}

pub fn render_connection_form(
    frame: &mut Frame, area: Rect, dialog: &FormDialog, protocols: &[ProtocolInfo], theme: &Theme,
) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let right = match list_mode(area.width) {
        Some(mode) => {
            let list = Rect::new(area.x, area.y, mode.width, area.height);
            render_protocol_list(frame, list, dialog, protocols, mode, theme);
            let divider_x = area.x + mode.width + 1;
            vertical_rule(frame, Rect::new(divider_x, area.y, 1, area.height), theme);
            let right_x = divider_x + 1 + GUTTER_AFTER_DIVIDER;
            Rect::new(right_x, area.y, area.right().saturating_sub(right_x), area.height)
        }
        None => area,
    };
    if right.width > 0 {
        render_fields(frame, right, dialog, list_mode(area.width).is_none(), theme);
    }
}

fn render_protocol_list(
    frame: &mut Frame, area: Rect, dialog: &FormDialog, protocols: &[ProtocolInfo], mode: ListMode, theme: &Theme,
) {
    frame
        .render_widget(Paragraph::new(Span::styled("protocol", theme.dim())), Rect::new(area.x, area.y, area.width, 1));
    let rule = Rect::new(area.x, area.y + 1, area.width, 1);
    hairline(frame, rule, theme);
    let list_focused = dialog.focused == 0;
    if list_focused {
        accent_underline(frame, rule, 0..area.width, theme);
    }

    let (choices, selected) = protocol_choices(dialog);
    let rows_per_item: u16 = if mode.descriptions { 2 } else { 1 };
    let visible = usize::from(area.height.saturating_sub(2) / rows_per_item).max(1);
    let offset = if selected >= visible { selected + 1 - visible } else { 0 };
    let selection_background = if list_focused { theme.focus_row() } else { theme.focus_row_inactive() };

    for (index, (id, label)) in choices.iter().enumerate().skip(offset) {
        let top = area.y + 2 + (index - offset) as u16 * rows_per_item;
        if top + rows_per_item > area.bottom() {
            break;
        }
        let is_selected = index == selected;
        let description = protocols
            .iter()
            .find(|info| info.id == id)
            .filter(|_| !label.contains(NOT_AVAILABLE))
            .map_or("", |info| info.description);
        let name_style = if is_selected { theme.focused_name() } else { theme.text() };
        let name = truncate_with_ellipsis(&printable(label), usize::from(area.width).saturating_sub(2));
        let marker = if is_selected { Span::styled("\u{203a} ", theme.accent_text()) } else { Span::raw("  ") };
        let name_line = Line::from(vec![marker, Span::styled(name, name_style)]);
        let name_area = Rect::new(area.x, top, area.width, 1);
        if is_selected {
            frame.buffer_mut().set_style(Rect::new(area.x, top, area.width, rows_per_item), selection_background);
        }
        frame.render_widget(Paragraph::new(name_line), name_area);
        if mode.descriptions {
            let text = truncate_with_ellipsis(description, usize::from(area.width).saturating_sub(2));
            let line = Line::from(vec![Span::raw("  "), Span::styled(text, theme.dim())]);
            frame.render_widget(Paragraph::new(line), Rect::new(area.x, top + 1, area.width, 1));
        }
    }
}

fn render_fields(frame: &mut Frame, area: Rect, dialog: &FormDialog, list_hidden: bool, theme: &Theme) {
    let (choices, selected) = protocol_choices(dialog);
    let title = choices.get(selected).map(|(_, label)| printable(label)).unwrap_or_default();
    let title_line = if list_hidden {
        let style = if dialog.focused == 0 { theme.bright() } else { theme.dim() };
        Line::from(Span::styled(format!("\u{25c0} {title} \u{25b6}"), style))
    } else {
        Line::from(Span::styled(title, theme.dim()))
    };
    frame.render_widget(Paragraph::new(title_line), Rect::new(area.x, area.y, area.width, 1));
    if area.height < 2 {
        return;
    }
    let rule = Rect::new(area.x, area.y + 1, area.width, 1);
    hairline(frame, rule, theme);
    accent_underline(frame, rule, 0..area.width, theme);

    let stack = Rect::new(area.x, area.y + 2, area.width, area.height - 2);
    render_field_stack(frame, stack, dialog, 1, theme);
}

#[cfg(test)]
mod tests;
