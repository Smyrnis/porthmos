use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    theme::Theme,
    widgets::{
        dialog::{FieldKind, FormDialog, FormField},
        history_view::printable,
        test_line,
        text_fit::{take_width, truncate_with_ellipsis},
    },
};

pub const LABEL_WIDTH: usize = 15;
const SAVED_SECRET: &str = "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022} (saved)";

struct Item {
    field: usize,
    top: usize,
    height: usize,
}

fn layout_items(dialog: &FormDialog, first_field: usize) -> Vec<Item> {
    let mut items = Vec::new();
    let mut top = 0;
    let texts: Vec<usize> =
        (first_field..dialog.fields.len()).filter(|index| !dialog.fields[*index].is_toggle()).collect();
    let toggles: Vec<usize> =
        (first_field..dialog.fields.len()).filter(|index| dialog.fields[*index].is_toggle()).collect();
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

fn notes_of(dialog: &FormDialog) -> usize {
    usize::from(dialog.hint.is_some()) + usize::from(dialog.error.is_some()) + usize::from(dialog.test.is_some())
}

pub fn stack_height(dialog: &FormDialog, first_field: usize) -> u16 {
    let fields = layout_items(dialog, first_field).last().map_or(0, |item| item.top + item.height);
    u16::try_from(fields + notes_of(dialog)).unwrap_or(u16::MAX)
}

pub fn displayed_value(field: &FormField) -> String {
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

pub fn render_field_stack(frame: &mut Frame, area: Rect, dialog: &FormDialog, first_field: usize, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let notes = notes_of(dialog);
    let viewport = usize::from(area.height).saturating_sub(notes);
    let items = layout_items(dialog, first_field);
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
        let y = area.y + (item.top - offset) as u16;
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
    if let Some(state) = &dialog.test {
        bottom -= 1;
        let line = test_line::line(state, usize::from(area.width), theme);
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, bottom, area.width, 1));
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
    let has_caret = matches!(field.kind, FieldKind::Text | FieldKind::Masked);
    let caret = (focused && has_caret).then_some(field.cursor);
    render_underline_value(
        frame,
        Rect::new(value_x, y, value_width as u16, 1),
        &displayed_value(field),
        caret,
        focused,
        theme,
    );
}

pub fn render_caret_value(frame: &mut Frame, area: Rect, value: &str, caret: Option<usize>, style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let (shown, caret_column) = match caret {
        Some(cursor) if value.width() > width => caret_window(value, cursor, width),
        _ => (truncate_with_ellipsis(value, width), prefix_width(value, caret.unwrap_or(0))),
    };
    frame.render_widget(Paragraph::new(Span::styled(shown, style)), Rect::new(area.x, area.y, area.width, 1));
    if caret.is_some() {
        let x = area.x + caret_column.min(width.saturating_sub(1)) as u16;
        frame.set_cursor_position(Position::new(x, area.y));
    }
}

pub fn render_underline_value(
    frame: &mut Frame, area: Rect, value: &str, caret: Option<usize>, focused: bool, theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let value_style = if focused { theme.bright() } else { theme.text() };
    render_caret_value(frame, area, value, caret, value_style);

    let underline_style = if focused { theme.accent_text() } else { theme.line_strong_text() };
    let underline = Rect::new(area.x, area.y + 1, area.width, 1).intersection(frame.area());
    if underline.width > 0 && underline.height > 0 {
        let rule = "\u{2500}".repeat(usize::from(underline.width));
        frame.render_widget(Paragraph::new(Span::styled(rule, underline_style)), underline);
    }
}

fn caret_window(text: &str, cursor: usize, width: usize) -> (String, usize) {
    let characters: Vec<char> = text.chars().collect();
    let cursor = cursor.min(characters.len());
    let room = width.saturating_sub(1);
    let mut start = cursor;
    let mut used = 0;
    while start > 0 {
        let character_width = characters[start - 1].width().unwrap_or(0);
        if used + character_width > room {
            break;
        }
        used += character_width;
        start -= 1;
    }
    let window: String = characters[start..].iter().collect();
    (take_width(&window, width), used)
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
