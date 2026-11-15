use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::diff::{DiffKind, DiffLine, DiffOutcome};
use porthmos_core::i18n;
use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        paging::PageRows,
        popup::{footer, render_popup},
        text_fit::truncate_with_ellipsis,
    },
};

const MIN_WIDTH: u16 = 40;
const MAX_WIDTH: u16 = 120;
const SCREEN_MARGIN: u16 = 2;
fn footer_hint() -> [(&'static str, String); 5] {
    [
        ("u", i18n::text("footer.overwrite")),
        ("m", i18n::text("footer.keep_copy")),
        ("c", i18n::text("footer.cancel")),
        ("esc", i18n::text("footer.back")),
        ("j/k", i18n::text("footer.scroll")),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffChoice {
    Overwrite,
    KeepCopy,
    Cancel,
    Back,
}

pub struct DiffDialog {
    title: String,
    rows: Vec<DiffLine>,
    note: Option<String>,
    scroll: usize,
    page: PageRows,
}

impl DiffDialog {
    pub fn new(name: &str, outcome: &DiffOutcome) -> Self {
        let rows = match outcome {
            DiffOutcome::Changes(lines) => lines.clone(),
            _ => Vec::new(),
        };
        Self {
            title: i18n::text_with("diff.title", &[("name", name)]),
            rows,
            note: outcome.note(),
            scroll: 0,
            page: PageRows::default(),
        }
    }

    fn last_scroll(&self) -> usize {
        self.rows.len().saturating_sub(self.page.get())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<DiffChoice> {
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return None;
        }
        let page = self.page.get();
        match key.code {
            KeyCode::Char('u') => return Some(DiffChoice::Overwrite),
            KeyCode::Char('m') => return Some(DiffChoice::KeepCopy),
            KeyCode::Char('c') => return Some(DiffChoice::Cancel),
            KeyCode::Esc | KeyCode::Char('q') => return Some(DiffChoice::Back),
            KeyCode::Char('j') | KeyCode::Down => self.scroll += 1,
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll += page,
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = self.last_scroll(),
            _ => {}
        }
        self.scroll = self.scroll.min(self.last_scroll());
        None
    }
}

fn row_style(kind: DiffKind) -> fn(&Theme) -> ratatui::style::Style {
    match kind {
        DiffKind::Context => Theme::text,
        DiffKind::Removed => Theme::err_text,
        DiffKind::Added => Theme::ok_text,
        DiffKind::Skipped(_) => Theme::dim,
    }
}

pub fn render_diff(frame: &mut Frame, area: Rect, dialog: &DiffDialog, theme: &Theme) {
    let width = area.width.saturating_sub(2 * SCREEN_MARGIN).clamp(MIN_WIDTH, MAX_WIDTH).min(area.width);
    let height = area.height.saturating_sub(SCREEN_MARGIN).min(area.height);
    let inner = render_popup(frame, area, (width, height), Some(&dialog.title), theme);
    if inner.width == 0 || inner.height < 2 {
        return;
    }
    let body_rows = inner.height.saturating_sub(2);
    dialog.page.remember(usize::from(body_rows));
    let body = Rect::new(inner.x, inner.y, inner.width, body_rows);
    let clip = usize::from(inner.width);
    let lines: Vec<Line> = match &dialog.note {
        Some(note) => vec![Line::styled(truncate_with_ellipsis(&printable(note), clip), theme.text())],
        None => dialog
            .rows
            .iter()
            .skip(dialog.scroll)
            .take(usize::from(body_rows))
            .map(|row| {
                let text = row.marked_text();
                Line::styled(truncate_with_ellipsis(&printable(&text), clip), row_style(row.kind)(theme))
            })
            .collect(),
    };
    frame.render_widget(Paragraph::new(lines), body);
    footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), &footer_hint(), theme);
}

#[cfg(test)]
mod tests;
