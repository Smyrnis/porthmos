use porthmos_core::ProtocolInfo;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    theme::Theme,
    widgets::{
        dialog::{FieldKind, FormDialog, FormField},
        frame::{accent_underline, hairline, vertical_rule},
        history_view::printable,
        text_fit::{truncate_start_with_ellipsis, truncate_with_ellipsis},
    },
};

const LABEL_WIDTH: usize = 15;
const WIDE_LIST: u16 = 42;
const MEDIUM_LIST: u16 = 32;
const NAMES_LIST: u16 = 22;
const MIN_WIDTH_WITH_DESCRIPTIONS: u16 = 80;
const MIN_WIDTH_WITH_LIST: u16 = 60;
const WIDE_LIST_MIN_WIDTH: u16 = 100;
const GUTTER_AFTER_DIVIDER: u16 = 2;
const SAVED_SECRET: &str = "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022} (saved)";
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

struct Item {
    field: usize,
    top: usize,
    height: usize,
}

fn layout_items(dialog: &FormDialog) -> Vec<Item> {
    let mut items = Vec::new();
    let mut top = 0;
    let texts: Vec<usize> = (1..dialog.fields.len()).filter(|index| !dialog.fields[*index].is_toggle()).collect();
    let toggles: Vec<usize> = (1..dialog.fields.len()).filter(|index| dialog.fields[*index].is_toggle()).collect();
    for field in texts.iter().copied() {
        items.push(Item { field, top, height: 2 });
        top += 2;
    }
    if !texts.is_empty() && !toggles.is_empty() {
        top += 1;
    }
    for field in toggles {
        items.push(Item { field, top, height: 1 });
        top += 1;
    }
    items
}

fn displayed_value(field: &FormField) -> String {
    match &field.kind {
        FieldKind::Text => printable(&field.value),
        FieldKind::Masked => "\u{2022}".repeat(field.value.chars().count()),
        FieldKind::SavedSecret => SAVED_SECRET.to_string(),
        FieldKind::Choice { choices, selected } => {
            let label = choices.get(*selected).map(|(_, label)| label.as_str()).unwrap_or("");
            format!("\u{25c0} {} \u{25b6}", printable(label))
        }
    }
}

fn prefix_width(text: &str, characters: usize) -> usize {
    text.chars().take(characters).map(|character| character.width().unwrap_or(0)).sum()
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

    let notes = u16::from(dialog.hint.is_some()) + u16::from(dialog.error.is_some());
    let viewport = usize::from(area.height.saturating_sub(2 + notes));
    let items = layout_items(dialog);
    let mut offset = 0;
    if let Some(focused) = items.iter().find(|item| item.field == dialog.focused) {
        let bottom = focused.top + focused.height;
        if bottom > viewport {
            offset = bottom - viewport;
        }
        offset = offset.min(focused.top);
    }

    let value_x = area.x + LABEL_WIDTH.min(usize::from(area.width)) as u16;
    let value_width = usize::from(area.width).saturating_sub(LABEL_WIDTH);
    for item in &items {
        if item.top < offset || item.top + item.height - offset > viewport {
            continue;
        }
        let y = area.y + 2 + (item.top - offset) as u16;
        let field = &dialog.fields[item.field];
        let focused = item.field == dialog.focused;
        if field.is_toggle() {
            render_toggle(frame, value_x, y, value_width, field, focused, theme);
        } else {
            render_text_field(frame, area, (value_x, value_width), y, field, focused, theme);
        }
    }

    let mut bottom = area.bottom();
    if let Some(error) = &dialog.error {
        bottom -= 1;
        let text = truncate_with_ellipsis(&printable(error), usize::from(area.width).saturating_sub(2));
        let line = Line::from(Span::styled(format!("\u{2022} {text}"), theme.err_text()));
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, bottom, area.width, 1));
    }
    if let Some(hint) = &dialog.hint {
        bottom -= 1;
        let text = truncate_with_ellipsis(&printable(hint), usize::from(area.width));
        frame.render_widget(Paragraph::new(Span::styled(text, theme.dim())), Rect::new(area.x, bottom, area.width, 1));
    }
}

fn render_text_field(
    frame: &mut Frame, area: Rect, value_column: (u16, usize), y: u16, field: &FormField, focused: bool, theme: &Theme,
) {
    let (value_x, value_width) = value_column;
    let label_style = if focused { theme.bright() } else { theme.dim() };
    let label = truncate_with_ellipsis(&printable(&field.label), LABEL_WIDTH - 1);
    frame.render_widget(Paragraph::new(Span::styled(label, label_style)), Rect::new(area.x, y, area.width, 1));
    if value_width == 0 {
        return;
    }

    let displayed = displayed_value(field);
    let overflows = displayed.width() > value_width;
    let shown = if overflows && focused {
        truncate_start_with_ellipsis(&displayed, value_width)
    } else {
        truncate_with_ellipsis(&displayed, value_width)
    };
    let value_style = if focused { theme.bright() } else { theme.text() };
    let value_area = Rect::new(value_x, y, value_width as u16, 1);
    frame.render_widget(Paragraph::new(Span::styled(shown.clone(), value_style)), value_area);

    let underline_style = if focused { theme.accent_text() } else { theme.line_strong_text() };
    let underline = "\u{2500}".repeat(value_width);
    frame.render_widget(
        Paragraph::new(Span::styled(underline, underline_style)),
        Rect::new(value_x, y + 1, value_width as u16, 1),
    );

    let has_caret = matches!(field.kind, FieldKind::Text | FieldKind::Masked);
    if focused && has_caret {
        let column = if overflows { shown.width() } else { prefix_width(&displayed, field.cursor) };
        let x = value_x + column.min(value_width.saturating_sub(1)) as u16;
        frame.set_cursor_position(Position::new(x, y));
    }
}

fn render_toggle(frame: &mut Frame, x: u16, y: u16, width: usize, field: &FormField, focused: bool, theme: &Theme) {
    if width < 4 {
        return;
    }
    let checked = field.submitted_value() == "true";
    let box_style = if checked { theme.accent_text() } else { theme.dim() };
    let label_style = if focused { theme.bright() } else { theme.text() };
    let label = truncate_with_ellipsis(&printable(&field.label), width - 4);
    let line = Line::from(vec![
        Span::styled(if checked { "[x]" } else { "[ ]" }, box_style),
        Span::raw(" "),
        Span::styled(label, label_style),
    ]);
    frame.render_widget(Paragraph::new(line), Rect::new(x, y, width as u16, 1));
}

#[cfg(test)]
mod tests;
