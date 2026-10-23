use std::ops::Range;

use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        text_fit::{take_width, truncate_with_ellipsis},
    },
};

const TAB_GAP: &str = "   ";
const DOT: &str = "\u{2022}";
const LOST: &str = "\u{d7}";

pub struct TabView<'a> {
    pub number: usize,
    pub name: &'a str,
    pub protocol: &'a str,
    pub active: bool,
    pub healthy: bool,
}

pub struct TabsLine {
    pub line: Line<'static>,
    pub active_columns: Option<Range<u16>>,
}

#[derive(Clone, Copy)]
enum Detail {
    Full,
    WithoutProtocol,
    ActiveNamed,
    ActiveOnly,
}

const DETAILS: [Detail; 4] = [Detail::Full, Detail::WithoutProtocol, Detail::ActiveNamed, Detail::ActiveOnly];

pub fn tabs_line(tabs: &[TabView<'_>], available_width: u16, theme: &Theme) -> TabsLine {
    let width = usize::from(available_width);
    if tabs.is_empty() || width == 0 {
        return TabsLine { line: Line::default(), active_columns: None };
    }
    for detail in DETAILS {
        let candidate = build(tabs, detail, theme);
        if candidate.line.width() <= width {
            return candidate;
        }
    }
    truncated_active_tab(tabs, width, theme)
}

fn marker(healthy: bool, theme: &Theme) -> Span<'static> {
    if healthy { Span::styled(DOT, theme.ok_text()) } else { Span::styled(LOST, theme.err_text()) }
}

fn push(spans: &mut Vec<Span<'static>>, column: &mut usize, span: Span<'static>) {
    *column += span.content.width();
    spans.push(span);
}

fn build(tabs: &[TabView<'_>], detail: Detail, theme: &Theme) -> TabsLine {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut column = 0usize;
    let mut active_columns = None;

    let shown = tabs.iter().filter(|tab| !matches!(detail, Detail::ActiveOnly) || tab.active);
    for (index, tab) in shown.enumerate() {
        if index > 0 {
            push(&mut spans, &mut column, Span::raw(TAB_GAP));
        }
        let start = column;
        let show_name = !matches!(detail, Detail::ActiveNamed) || tab.active;
        push(&mut spans, &mut column, Span::styled(tab.number.to_string(), theme.dim()));
        if show_name {
            let name_style = if tab.active { theme.bright() } else { theme.muted() };
            push(&mut spans, &mut column, Span::raw(" "));
            push(&mut spans, &mut column, Span::styled(printable(tab.name), name_style));
        }
        if tab.active {
            active_columns = Some(start as u16..column as u16);
        }
        if matches!(detail, Detail::Full) && !tab.protocol.is_empty() {
            push(&mut spans, &mut column, Span::raw(" "));
            push(&mut spans, &mut column, Span::styled(printable(tab.protocol), theme.dim()));
        }
        push(&mut spans, &mut column, Span::raw(" "));
        push(&mut spans, &mut column, marker(tab.healthy, theme));
    }
    TabsLine { line: Line::from(spans), active_columns }
}

fn truncated_active_tab(tabs: &[TabView<'_>], width: usize, theme: &Theme) -> TabsLine {
    let Some(tab) = tabs.iter().find(|tab| tab.active) else {
        return TabsLine { line: Line::default(), active_columns: None };
    };
    let number = tab.number.to_string();
    let prefix_width = number.width() + 1;
    let suffix_width = 1 + DOT.width();
    let Some(budget) = width.checked_sub(prefix_width + suffix_width).filter(|budget| *budget > 0) else {
        let hard_cut: String = take_width(&format!("{number} {DOT}"), width);
        let columns = (number.width().min(width)) as u16;
        return TabsLine { line: Line::from(Span::styled(hard_cut, theme.dim())), active_columns: Some(0..columns) };
    };
    let name = truncate_with_ellipsis(&printable(tab.name), budget);
    let name_width = name.width();
    let spans = vec![
        Span::styled(number, theme.dim()),
        Span::raw(" "),
        Span::styled(name, theme.bright()),
        Span::raw(" "),
        marker(tab.healthy, theme),
    ];
    TabsLine { line: Line::from(spans), active_columns: Some(0..(prefix_width - 1 + 1 + name_width) as u16) }
}

#[cfg(test)]
mod tests;
