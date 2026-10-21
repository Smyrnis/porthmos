use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        popup::{footer, render_popup},
        text_fit::{pad_right, truncate_with_ellipsis},
    },
};

const KEY_COLUMN: usize = 12;
const MAX_WIDTH: u16 = 64;
const FOOTER_ROWS: u16 = 2;
const BORDER_ROWS: u16 = 2;
const UNBOUND: &str = "unbound";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpRow {
    pub key: Option<String>,
    pub action: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpSection {
    pub title: &'static str,
    pub rows: Vec<HelpRow>,
}

fn lines_of(sections: &[HelpSection], width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            lines.push(Line::default());
        }
        lines.push(Line::from(Span::styled(section.title, theme.dim())));
        for row in &section.rows {
            let key = match &row.key {
                Some(key) => Span::styled(
                    pad_right(&truncate_with_ellipsis(&printable(key), KEY_COLUMN - 1), KEY_COLUMN),
                    theme.bright(),
                ),
                None => Span::styled(pad_right(UNBOUND, KEY_COLUMN), theme.dim()),
            };
            let action = truncate_with_ellipsis(row.action, width.saturating_sub(KEY_COLUMN));
            lines.push(Line::from(vec![key, Span::styled(action, theme.text())]));
        }
    }
    lines
}

pub fn render_help(frame: &mut Frame, area: Rect, sections: &[HelpSection], scroll: usize, theme: &Theme) -> usize {
    let width = MAX_WIDTH.min(area.width);
    let lines = lines_of(sections, usize::from(width.saturating_sub(4)), theme);
    let height =
        u16::try_from(lines.len()).unwrap_or(u16::MAX).saturating_add(FOOTER_ROWS + BORDER_ROWS).min(area.height);
    let inner = render_popup(frame, area, (width, height), Some("help"), theme);
    if inner.width == 0 || inner.height == 0 {
        return 0;
    }

    let visible = usize::from(inner.height.saturating_sub(FOOTER_ROWS));
    let max_scroll = lines.len().saturating_sub(visible);
    let shown: Vec<Line> = lines.into_iter().skip(scroll.min(max_scroll)).take(visible).collect();
    frame.render_widget(Paragraph::new(shown), Rect::new(inner.x, inner.y, inner.width, visible as u16));
    footer(
        frame,
        Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
        &[("\u{2191}\u{2193}", "scroll"), ("any key", "close")],
        theme,
    );
    max_scroll
}

#[cfg(test)]
mod tests;
