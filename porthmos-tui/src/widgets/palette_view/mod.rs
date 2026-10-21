use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    palette::{Entry, Group, Hit, Palette},
    theme::Theme,
    widgets::{
        form_fields::render_caret_value,
        frame::hairline,
        history_view::printable,
        popup::render_popup_at,
        text_fit::{ELLIPSIS, truncate_with_ellipsis},
    },
};

const POPUP_WIDTH: u16 = 60;
const CHROME_ROWS: usize = 6;
const MIN_FOOTER_INNER_HEIGHT: u16 = 5;
const MARKER_WIDTH: usize = 2;
const HINT_GAP: usize = 2;
const COUNT_GAP: usize = 3;
const MOVE_GAP: usize = 2;
const PREFIX_GAP: usize = 1;
const FOOTER_SEPARATION: usize = 2;
const MOVE_PAIRS: [(&str, &str); 3] = [("\u{2191}\u{2193}", "move"), ("enter", "run"), ("esc", "close")];
const PREFIX_PAIRS: [(&str, &str); 3] = [(">", "actions"), ("@", "hosts"), ("#", "files")];

enum Row {
    Title(Group),
    Blank,
    Hit(usize),
}

fn rows_of(palette: &Palette) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut current: Option<Group> = None;
    for (index, hit) in palette.hits().iter().enumerate() {
        let group = palette.entry(hit).group;
        if current != Some(group) {
            if current.is_some() {
                rows.push(Row::Blank);
            }
            rows.push(Row::Title(group));
            current = Some(group);
        }
        rows.push(Row::Hit(index));
    }
    rows
}

fn popup_rect(screen: Rect, rows: usize) -> Rect {
    let width = u16::try_from(u32::from(screen.width) * 9 / 10).unwrap_or(u16::MAX).min(POPUP_WIDTH);
    let x = screen.x + (screen.width - width) / 2;
    let y = screen.y + screen.height / 6;
    let wanted = u16::try_from(rows.max(1) + CHROME_ROWS).unwrap_or(u16::MAX);
    let cap = u16::try_from(u32::from(screen.height) * 60 / 100).unwrap_or(u16::MAX);
    let height = wanted.min(cap).min(screen.bottom().saturating_sub(y));
    Rect::new(x, y, width, height)
}

fn render_input(frame: &mut Frame, area: Rect, palette: &Palette, theme: &Theme) {
    let width = usize::from(area.width);
    let count = palette.hits().len().to_string();
    let show_count = width >= MARKER_WIDTH + 6 + COUNT_GAP + count.width();
    frame.render_widget(Paragraph::new(Span::styled("\u{203a}", theme.accent_text())), area);
    let reserved = if show_count { count.width() + COUNT_GAP } else { 0 };
    let query_width = width.saturating_sub(MARKER_WIDTH + reserved);
    if query_width > 0 {
        let query_area = Rect::new(area.x + MARKER_WIDTH as u16, area.y, query_width as u16, 1);
        render_caret_value(frame, query_area, &printable(palette.query()), Some(palette.caret()), theme.bright());
    }
    if show_count {
        let x = area.x + area.width - count.width() as u16;
        frame.render_widget(
            Paragraph::new(Span::styled(count.clone(), theme.dim())),
            Rect::new(x, area.y, count.width() as u16, 1),
        );
    }
}

fn label_spans(
    label: &str, matched: &[usize], budget: usize, base: ratatui::style::Style, theme: &Theme,
) -> Vec<Span<'static>> {
    let total = label.width();
    let allowed = if total > budget { budget.saturating_sub(1) } else { budget };
    let mut spans = Vec::new();
    let mut used = 0;
    for (index, character) in label.chars().enumerate() {
        let width = character.width().unwrap_or(0);
        if used + width > allowed {
            break;
        }
        used += width;
        let style =
            if matched.binary_search(&index).is_ok() { theme.accent_text().patch(base_modifiers(base)) } else { base };
        spans.push(Span::styled(character.to_string(), style));
    }
    if total > budget && budget > 0 {
        spans.push(Span::styled(ELLIPSIS.to_string(), base));
    }
    spans
}

fn base_modifiers(base: ratatui::style::Style) -> ratatui::style::Style {
    ratatui::style::Style::default().add_modifier(base.add_modifier)
}

fn render_hit(frame: &mut Frame, area: Rect, hit: &Hit, entry: &Entry, selected: bool, theme: &Theme) {
    if selected {
        frame.buffer_mut().set_style(area, theme.focus_row());
    }
    let width = usize::from(area.width);
    let hint = entry.hint.as_deref().map(|hint| truncate_with_ellipsis(&printable(hint), width / 2));
    let hint_width = hint.as_ref().map_or(0, |hint| hint.width());
    let reserved = if hint_width > 0 { hint_width + HINT_GAP } else { 0 };
    let budget = width.saturating_sub(MARKER_WIDTH + reserved);
    let base = if selected { theme.focused_name() } else { theme.text() };
    let marker = if selected { Span::styled("\u{203a} ", theme.accent_text()) } else { Span::raw("  ") };

    let mut spans = vec![marker];
    spans.extend(label_spans(&printable(&entry.label), &hit.matched, budget, base, theme));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    if let Some(hint) = hint.filter(|_| hint_width > 0 && hint_width < width) {
        let x = area.x + area.width - hint_width as u16;
        frame
            .render_widget(Paragraph::new(Span::styled(hint, theme.dim())), Rect::new(x, area.y, hint_width as u16, 1));
    }
}

fn render_results(frame: &mut Frame, area: Rect, palette: &Palette, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if palette.hits().is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("no matches", theme.dim())),
            Rect::new(area.x, area.y, area.width, 1),
        );
        return;
    }
    let rows = rows_of(palette);
    let visible = usize::from(area.height);
    let selected_row =
        rows.iter().position(|row| matches!(row, Row::Hit(index) if *index == palette.selected())).unwrap_or(0);
    let offset = (selected_row + 1).saturating_sub(visible);
    for (position, row) in rows.iter().enumerate().skip(offset).take(visible) {
        let row_area = Rect::new(area.x, area.y + (position - offset) as u16, area.width, 1);
        match row {
            Row::Blank => {}
            Row::Title(group) => {
                frame.render_widget(Paragraph::new(Span::styled(group.title(), theme.dim())), row_area);
            }
            Row::Hit(index) => {
                let hit = &palette.hits()[*index];
                render_hit(frame, row_area, hit, palette.entry(hit), *index == palette.selected(), theme);
            }
        }
    }
}

fn pairs_line(pairs: &[(&str, &str)], gap: usize, width: usize, theme: &Theme) -> Option<(Line<'static>, usize)> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for (key, label) in pairs {
        let pair_width = key.width() + 1 + label.width();
        let separation = if spans.is_empty() { 0 } else { gap };
        if used + separation + pair_width > width {
            break;
        }
        if separation > 0 {
            spans.push(Span::raw(" ".repeat(separation)));
        }
        spans.push(Span::styled(key.to_string(), theme.bright()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(label.to_string(), theme.dim()));
        used += separation + pair_width;
    }
    (!spans.is_empty()).then(|| (Line::from(spans), used))
}

fn render_footer(frame: &mut Frame, area: Rect, theme: &Theme) {
    let width = usize::from(area.width);
    let left_full = pairs_line(&MOVE_PAIRS, MOVE_GAP, usize::MAX, theme).map_or(0, |(_, used)| used);
    let right_full = pairs_line(&PREFIX_PAIRS, PREFIX_GAP, usize::MAX, theme).map_or(0, |(_, used)| used);
    if let Some((line, _)) = pairs_line(&MOVE_PAIRS, MOVE_GAP, width, theme) {
        frame.render_widget(Paragraph::new(line), area);
    }
    if left_full + FOOTER_SEPARATION + right_full <= width
        && let Some((line, used)) = pairs_line(&PREFIX_PAIRS, PREFIX_GAP, width, theme)
    {
        let x = area.x + area.width - used as u16;
        frame.render_widget(Paragraph::new(line), Rect::new(x, area.y, used as u16, 1));
    }
}

pub fn render_palette(frame: &mut Frame, screen: Rect, palette: &Palette, theme: &Theme) {
    let rows = rows_of(palette).len();
    let inner = render_popup_at(frame, screen, popup_rect(screen, rows), None, theme);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    render_input(frame, Rect::new(inner.x, inner.y, inner.width, 1), palette, theme);
    if inner.height < 2 {
        return;
    }
    hairline(frame, Rect::new(inner.x, inner.y + 1, inner.width, 1), theme);

    let with_footer = inner.height >= MIN_FOOTER_INNER_HEIGHT;
    let results_top = inner.y + 2;
    let results_bottom = if with_footer { inner.bottom() - 2 } else { inner.bottom() };
    let results = Rect::new(inner.x, results_top, inner.width, results_bottom.saturating_sub(results_top));
    render_results(frame, results, palette, theme);
    if with_footer {
        hairline(frame, Rect::new(inner.x, inner.bottom() - 2, inner.width, 1), theme);
        render_footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), theme);
    }
}

#[cfg(test)]
mod tests;
