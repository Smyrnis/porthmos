use std::sync::{Arc, Mutex};

use alacritty_terminal::{
    event::{Event, EventListener},
    grid::Dimensions,
    grid::Scroll,
    index::{Column, Line, Point, Side as GridSide},
    selection::{Selection, SelectionType},
    term::{Config, Osc52, Term, TermMode, cell::Flags},
    vte::{
        self,
        ansi::{Color as AnsiColor, CursorShape as AnsiCursorShape, NamedColor, Processor, StdSyncHandler},
    },
};

use crate::{
    Cell, CellWidth, Color, Cursor, CursorShape, GridPoint, Match, Modes, MouseMode, SearchDirection, SelectionKind,
    Side, Snapshot, Style, TermSize, TerminalEvent, ViewPoint,
    events::{parse_working_directory, sanitize_title},
    osc_guard::OscGuard,
    search,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Feed {
    pub replies: Vec<u8>,
    pub events: Vec<TerminalEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalConfig {
    pub scrollback: usize,
    pub allow_clipboard_store: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self { scrollback: 10_000, allow_clipboard_store: false }
    }
}

struct GridSize(TermSize);

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        usize::from(self.0.rows)
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.0.rows)
    }

    fn columns(&self) -> usize {
        usize::from(self.0.columns)
    }
}

#[derive(Default)]
struct Pending {
    replies: Vec<u8>,
    events: Vec<TerminalEvent>,
}

#[derive(Clone, Default)]
struct Collector(Arc<Mutex<Pending>>);

impl EventListener for Collector {
    fn send_event(&self, event: Event) {
        let mut pending = self.0.lock().unwrap();
        match event {
            Event::PtyWrite(text) => pending.replies.extend_from_slice(text.as_bytes()),
            Event::Bell => pending.events.push(TerminalEvent::Bell),
            Event::Title(title) => pending.events.push(TerminalEvent::Title(sanitize_title(&title))),
            Event::ResetTitle => pending.events.push(TerminalEvent::ResetTitle),
            Event::ClipboardStore(_, text) => pending.events.push(TerminalEvent::ClipboardStore(text)),
            _ => {}
        }
    }
}

#[derive(Default)]
struct WorkingDirectoryScanner {
    found: Vec<TerminalEvent>,
}

impl vte::Perform for WorkingDirectoryScanner {
    fn osc_dispatch(&mut self, params: &[&[u8]], _: bool) {
        if params.first() != Some(&&b"7"[..]) || params.len() < 2 {
            return;
        }
        let uri = params[1..].iter().map(|part| String::from_utf8_lossy(part)).collect::<Vec<_>>().join(";");
        if let Some(directory) = parse_working_directory(&uri) {
            self.found.push(TerminalEvent::WorkingDirectory(directory));
        }
    }
}

pub struct Terminal {
    term: Term<Collector>,
    processor: Processor<StdSyncHandler>,
    scanner: vte::Parser,
    guard: OscGuard,
    collector: Collector,
    size: TermSize,
}

impl Terminal {
    pub fn new(size: TermSize, config: TerminalConfig) -> Self {
        let size = TermSize::clamped(size.columns, size.rows);
        let osc52 = if config.allow_clipboard_store { Osc52::OnlyCopy } else { Osc52::Disabled };
        let settings = Config { scrolling_history: config.scrollback, osc52, ..Config::default() };
        let collector = Collector::default();
        Self {
            term: Term::new(settings, &GridSize(size), collector.clone()),
            processor: Processor::new(),
            scanner: vte::Parser::new(),
            guard: OscGuard::new(),
            collector,
            size,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Feed {
        let bytes = self.guard.filter(bytes);
        let mut scan = WorkingDirectoryScanner::default();
        self.scanner.advance(&mut scan, &bytes);
        self.processor.advance(&mut self.term, &bytes);
        let mut pending = std::mem::take(&mut *self.collector.0.lock().unwrap());
        scan.found.append(&mut pending.events);
        Feed { replies: pending.replies, events: scan.found }
    }

    pub fn resize(&mut self, size: TermSize) {
        self.size = TermSize::clamped(size.columns, size.rows);
        self.term.resize(GridSize(self.size));
    }

    pub fn size(&self) -> TermSize {
        self.size
    }

    pub fn snapshot(&self) -> Snapshot {
        let grid = self.term.grid();
        let offset = grid.display_offset();
        let selection = self.term.selection.as_ref().and_then(|selection| selection.to_range(&self.term));
        let mut cells = Vec::with_capacity(usize::from(self.size.columns) * usize::from(self.size.rows));
        for row in 0..i32::from(self.size.rows) {
            for column in 0..usize::from(self.size.columns) {
                let point = Point::new(Line(row - offset as i32), Column(column));
                let mut cell = convert_cell(&grid[point]);
                cell.selected = selection.is_some_and(|range| range.contains(point));
                cells.push(cell);
            }
        }
        Snapshot {
            size: self.size,
            cells,
            cursor: self.cursor(offset),
            display_offset: offset,
            history: self.term.history_size(),
            modes: convert_modes(*self.term.mode()),
        }
    }

    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    pub fn start_selection(&mut self, kind: SelectionKind, point: ViewPoint, side: Side) {
        let kind = match kind {
            SelectionKind::Simple => SelectionType::Simple,
            SelectionKind::Words => SelectionType::Semantic,
            SelectionKind::Lines => SelectionType::Lines,
            SelectionKind::Block => SelectionType::Block,
        };
        self.term.selection = Some(Selection::new(kind, self.view_point(point), grid_side(side)));
    }

    pub fn update_selection(&mut self, point: ViewPoint, side: Side) {
        let point = self.view_point(point);
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, grid_side(side));
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    pub fn selected_text(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    pub fn search(&self, query: &str, from: GridPoint, direction: SearchDirection) -> Option<Match> {
        search::find(&self.term, query, from, direction)
    }

    pub fn reveal(&mut self, found: Match) {
        let point = search::to_point(&self.term, found.start);
        self.term.scroll_to_point(point);
    }

    fn view_point(&self, point: ViewPoint) -> Point {
        let row = i32::from(point.row.min(self.size.rows - 1));
        let column = usize::from(point.column.min(self.size.columns - 1));
        Point::new(Line(row - self.term.grid().display_offset() as i32), Column(column))
    }

    fn cursor(&self, offset: usize) -> Cursor {
        let point = self.term.grid().cursor.point;
        let style = self.term.cursor_style();
        let shape = match style.shape {
            AnsiCursorShape::Underline => CursorShape::Underline,
            AnsiCursorShape::Beam => CursorShape::Bar,
            _ => CursorShape::Block,
        };
        Cursor {
            row: point.line.0.clamp(0, i32::from(self.size.rows) - 1) as u16,
            column: point.column.0.min(usize::from(self.size.columns) - 1) as u16,
            shape,
            visible: offset == 0
                && style.shape != AnsiCursorShape::Hidden
                && self.term.mode().contains(TermMode::SHOW_CURSOR),
        }
    }
}

fn grid_side(side: Side) -> GridSide {
    match side {
        Side::Left => GridSide::Left,
        Side::Right => GridSide::Right,
    }
}

fn convert_color(color: AnsiColor) -> Color {
    match color {
        AnsiColor::Spec(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(index) => Color::Indexed(index),
        AnsiColor::Named(name) => match name as usize {
            ansi @ 0..=15 => Color::Indexed(ansi as u8),
            _ => match name {
                NamedColor::DimBlack
                | NamedColor::DimRed
                | NamedColor::DimGreen
                | NamedColor::DimYellow
                | NamedColor::DimBlue
                | NamedColor::DimMagenta
                | NamedColor::DimCyan
                | NamedColor::DimWhite => Color::Indexed((name as usize - NamedColor::DimBlack as usize) as u8),
                _ => Color::Default,
            },
        },
    }
}

fn convert_style(flags: Flags) -> Style {
    let mapping = [
        (Flags::BOLD, Style::BOLD),
        (Flags::DIM, Style::DIM),
        (Flags::ITALIC, Style::ITALIC),
        (Flags::ALL_UNDERLINES, Style::UNDERLINE),
        (Flags::INVERSE, Style::INVERSE),
        (Flags::STRIKEOUT, Style::STRIKEOUT),
        (Flags::HIDDEN, Style::HIDDEN),
    ];
    mapping
        .into_iter()
        .filter(|(flag, _)| flags.intersects(*flag))
        .fold(Style::default(), |all, (_, style)| all | style)
}

fn convert_cell(cell: &alacritty_terminal::term::cell::Cell) -> Cell {
    let width = if cell.flags.contains(Flags::WIDE_CHAR) {
        CellWidth::Wide
    } else if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
        CellWidth::Spacer
    } else {
        CellWidth::Single
    };
    Cell {
        character: cell.c,
        combining: cell.zerowidth().map(|marks| marks.to_vec().into_boxed_slice()),
        width,
        foreground: convert_color(cell.fg),
        background: convert_color(cell.bg),
        style: convert_style(cell.flags),
        hyperlink: cell.hyperlink().map(|link| link.uri().into()),
        selected: false,
    }
}

fn convert_modes(mode: TermMode) -> Modes {
    let mouse = if mode.contains(TermMode::MOUSE_MOTION) {
        MouseMode::Motion
    } else if mode.contains(TermMode::MOUSE_DRAG) {
        MouseMode::Drag
    } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
        MouseMode::Click
    } else {
        MouseMode::Off
    };
    Modes {
        application_cursor: mode.contains(TermMode::APP_CURSOR),
        application_keypad: mode.contains(TermMode::APP_KEYPAD),
        bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
        mouse,
        mouse_sgr: mode.contains(TermMode::SGR_MOUSE),
        alternate_screen: mode.contains(TermMode::ALT_SCREEN),
    }
}

#[cfg(test)]
mod tests;
