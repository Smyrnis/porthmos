use chrono::{DateTime, Local};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        panel_view::{PanelView, Row},
        text_fit::truncate_with_ellipsis,
    },
};

pub mod dates;

use dates::format_modified;

const MARKER_WIDTH: usize = 2;
const COLUMN_GAP: usize = 1;
const SIZE_WIDTH: usize = 8;
const MODIFIED_WIDTH: usize = 11;
const MIN_NAME_WIDTH: usize = 4;
const MIN_WIDTH_WITH_SIZE: u16 = 28;

pub struct ListContext<'a> {
    pub active: bool,
    pub show_modified: bool,
    pub now: &'a DateTime<Local>,
}

#[derive(Debug, Clone, Copy)]
struct Columns {
    name_width: usize,
    show_size: bool,
    show_modified: bool,
}

impl Columns {
    fn for_width(width: u16, show_modified: bool) -> Self {
        let show_size = width >= MIN_WIDTH_WITH_SIZE;
        let mut overhead = MARKER_WIDTH;
        if show_size {
            overhead += COLUMN_GAP + SIZE_WIDTH;
        }
        if show_modified {
            overhead += COLUMN_GAP + MODIFIED_WIDTH;
        }
        Columns {
            name_width: usize::from(width).saturating_sub(overhead).max(MIN_NAME_WIDTH),
            show_size,
            show_modified,
        }
    }
}

fn pad_right(text: &str, width: usize) -> String {
    format!("{text}{}", " ".repeat(width.saturating_sub(text.width())))
}

fn pad_left(text: &str, width: usize) -> String {
    format!("{}{text}", " ".repeat(width.saturating_sub(text.width())))
}

pub fn render_file_list(frame: &mut Frame, area: Rect, panel: &PanelView, context: &ListContext<'_>, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let columns = Columns::for_width(area.width, context.show_modified);
    let header = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(Paragraph::new(Line::from(Span::styled(header_text(columns), theme.dim()))), header);

    let body = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
    if body.height == 0 {
        return;
    }
    let items: Vec<ListItem> = panel
        .rows()
        .iter()
        .enumerate()
        .map(|(index, row)| ListItem::new(row_line(row, index == panel.cursor, panel, context, columns, theme)))
        .collect();
    let highlight = if context.active { theme.focus_row() } else { theme.focus_row_inactive() };
    let list = List::new(items).highlight_style(highlight);

    let mut state = ListState::default();
    if !panel.rows().is_empty() {
        state.select(Some(panel.cursor));
    }
    frame.render_stateful_widget(list, body, &mut state);
}

fn header_text(columns: Columns) -> String {
    let mut text = format!("{}{}", " ".repeat(MARKER_WIDTH), pad_right("name", columns.name_width));
    if columns.show_size {
        text.push_str(&format!("{}{}", " ".repeat(COLUMN_GAP), pad_left("size", SIZE_WIDTH)));
    }
    if columns.show_modified {
        text.push_str(&format!("{}{}", " ".repeat(COLUMN_GAP), pad_right("modified", MODIFIED_WIDTH)));
    }
    text
}

fn row_line(
    row: &Row, focused: bool, panel: &PanelView, context: &ListContext<'_>, columns: Columns, theme: &Theme,
) -> Line<'static> {
    let selected = matches!(row, Row::Entry(entry) if panel.selected.contains(&entry.path));
    let marker = if focused && context.active {
        Span::styled("\u{203a}", theme.accent_text())
    } else if selected {
        Span::styled("+", theme.accent_text())
    } else {
        Span::raw(" ")
    };

    let (name, style) = match row {
        Row::Parent => ("..".to_string(), theme.dim()),
        Row::Entry(entry) => (name_cell(entry), entry_style(entry.is_dir, context.active, theme)),
    };
    let style = if focused && context.active { theme.focused_name() } else { style };
    let name = pad_right(&truncate_name(&name, columns.name_width), columns.name_width);

    let mut spans = vec![marker, Span::raw(" "), Span::styled(name, style)];
    if let Row::Entry(entry) = row {
        if columns.show_size {
            let size = if entry.is_dir { String::new() } else { size_label(entry.size) };
            spans.push(Span::styled(format!(" {}", pad_left(&size, SIZE_WIDTH)), theme.dim()));
        }
        if columns.show_modified {
            let modified = format_modified(entry.modified, context.now);
            spans.push(Span::styled(format!(" {}", pad_right(&modified, MODIFIED_WIDTH)), theme.dim()));
        }
    }
    Line::from(spans)
}

fn entry_style(is_dir: bool, active: bool, theme: &Theme) -> Style {
    match (active, is_dir) {
        (true, true) => theme.directory_name(),
        (true, false) => theme.text(),
        (false, true) => theme.inactive_dir_name(),
        (false, false) => theme.inactive_file_name(),
    }
}

pub(crate) fn name_cell(entry: &porthmos_core::Entry) -> String {
    let name = printable(&entry.name);
    if entry.is_dir {
        format!("{name}/")
    } else if entry.permissions.is_some_and(|mode| mode & 0o111 != 0) {
        format!("{name}*")
    } else {
        name
    }
}

pub(crate) fn permissions_string(entry: &porthmos_core::Entry) -> Option<String> {
    let mode = entry.permissions?;
    let kind = if entry.is_dir { 'd' } else { '-' };
    Some(format!("{kind}{}", permission_triplets(mode)))
}

pub fn size_label(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while (value >= 1024.0 || (value * 10.0).round() / 10.0 >= 1000.0) && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

pub(crate) fn truncate_name(name: &str, max_width: usize) -> String {
    if max_width <= 1 && name.width() > max_width {
        return "\u{2026}".to_string();
    }
    truncate_with_ellipsis(name, max_width)
}

pub fn format_size(size: u64, is_dir: bool) -> String {
    if is_dir {
        return "\u{2014}".to_string();
    }
    if size < 1024 {
        return size.to_string();
    }

    const UNITS: [&str; 4] = ["K", "M", "G", "T"];
    let mut value = size as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1}{}", UNITS[unit])
}

fn permission_triplets(mode: u32) -> String {
    const BITS: [(u32, char); 9] = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];

    BITS.iter().map(|(bit, ch)| if mode & bit != 0 { *ch } else { '-' }).collect()
}

#[cfg(test)]
mod tests;
