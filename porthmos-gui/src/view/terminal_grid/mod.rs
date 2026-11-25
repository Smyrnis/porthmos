use iced::{
    Border, Color, Element, Font, Length, Pixels, Point, Rectangle, Shadow, Size,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, renderer,
        renderer::Quad,
        text::{self, Paragraph as _},
        widget::Tree,
    },
    alignment::Vertical,
    font::{Style as FontStyle, Weight},
};
use porthmos_term::{Cell, CellWidth, CursorShape, MouseButton, Side, Snapshot, Style, TermSize, ViewPoint};

use crate::drawer::pointer::{PointerKind, cell_at, wheel_lines};

use crate::theme::{Palette, ansi::resolve, mix};

const TEXT_SIZE: f32 = 13.0;
pub const LINE_HEIGHT: f32 = 18.0;
const DIM_BLEND: f32 = 0.45;
const CURSOR_FILL_ALPHA: f32 = 0.55;
const CURSOR_BAR_WIDTH: f32 = 2.0;
const DECORATION_THICKNESS: f32 = 1.0;
const FALLBACK_WIDTH_RATIO: f32 = 0.6;
const SCROLL_THUMB_MIN: f32 = 12.0;
const SCROLL_THUMB_WIDTH: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    pub width: f32,
    pub height: f32,
}

pub fn grid_size(pixels: Size, metrics: CellMetrics) -> TermSize {
    let cells = |length: f32, cell: f32| -> u16 {
        if !length.is_finite() || !cell.is_finite() || cell <= 0.0 || length <= 0.0 {
            return 1;
        }
        (length / cell).floor().clamp(1.0, f32::from(u16::MAX)) as u16
    };
    TermSize::clamped(cells(pixels.width, metrics.width), cells(pixels.height, metrics.height))
}

#[derive(Debug, Default)]
pub struct ResizeGate {
    owner: Option<u64>,
    last: Option<TermSize>,
}

impl ResizeGate {
    pub fn changed(&mut self, size: TermSize) -> Option<TermSize> {
        if self.last == Some(size) {
            return None;
        }
        self.last = Some(size);
        Some(size)
    }

    pub fn changed_for(&mut self, owner: u64, size: TermSize) -> Option<TermSize> {
        if self.owner != Some(owner) {
            self.owner = Some(owner);
            self.last = None;
        }
        self.changed(size)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub column: u16,
    pub text: String,
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundSpan {
    pub column: u16,
    pub width: u16,
    pub color: Color,
}

fn cell_colours(cell: &Cell, palette: &Palette) -> (Color, Color) {
    let mut foreground = resolve(cell.foreground, true, palette);
    let mut background = resolve(cell.background, false, palette);
    if cell.style.contains(Style::INVERSE) {
        std::mem::swap(&mut foreground, &mut background);
    }
    if cell.style.contains(Style::DIM) {
        foreground = mix(foreground, background, DIM_BLEND);
    }
    if cell.selected {
        foreground = palette.fg_focus;
        background = palette.row_focus;
    }
    (foreground, background)
}

pub fn row_backgrounds(cells: &[Cell], palette: &Palette) -> Vec<BackgroundSpan> {
    let mut spans: Vec<BackgroundSpan> = Vec::new();
    for (index, cell) in cells.iter().enumerate() {
        let (_, background) = cell_colours(cell, palette);
        if background == palette.bg {
            continue;
        }
        let column = index as u16;
        match spans.last_mut() {
            Some(last) if last.color == background && last.column + last.width == column => last.width += 1,
            _ => spans.push(BackgroundSpan { column, width: 1, color: background }),
        }
    }
    spans
}

struct Look {
    color: Color,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
}

fn look_of(cell: &Cell, palette: &Palette) -> Look {
    let (color, _) = cell_colours(cell, palette);
    Look {
        color,
        bold: cell.style.contains(Style::BOLD),
        italic: cell.style.contains(Style::ITALIC),
        underline: cell.style.contains(Style::UNDERLINE),
        strikeout: cell.style.contains(Style::STRIKEOUT),
    }
}

fn same_look(left: &Look, right: &Look) -> bool {
    left.color == right.color
        && left.bold == right.bold
        && left.italic == right.italic
        && left.underline == right.underline
        && left.strikeout == right.strikeout
}

fn finished(column: u16, text: String, look: Look) -> Option<TextRun> {
    let decorated = look.underline || look.strikeout;
    let (column, text) = if decorated {
        (column, text)
    } else {
        let leading = text.chars().take_while(|character| *character == ' ').count();
        (column + leading as u16, text.trim().to_string())
    };
    if text.is_empty() {
        return None;
    }
    Some(TextRun {
        column,
        text,
        color: look.color,
        bold: look.bold,
        italic: look.italic,
        underline: look.underline,
        strikeout: look.strikeout,
    })
}

pub fn row_runs(cells: &[Cell], palette: &Palette) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut open: Option<(u16, String, Look)> = None;
    let close = |open: &mut Option<(u16, String, Look)>, runs: &mut Vec<TextRun>| {
        if let Some((column, text, look)) = open.take() {
            runs.extend(finished(column, text, look));
        }
    };
    for (index, cell) in cells.iter().enumerate() {
        if cell.width == CellWidth::Spacer {
            continue;
        }
        let column = index as u16;
        if cell.style.contains(Style::HIDDEN) {
            close(&mut open, &mut runs);
            continue;
        }
        let look = look_of(cell, palette);
        let plain = cell.character.is_ascii() && cell.combining.is_none() && cell.width == CellWidth::Single;
        if !plain {
            close(&mut open, &mut runs);
            let mut text = cell.character.to_string();
            if let Some(marks) = &cell.combining {
                text.extend(marks.iter());
            }
            runs.extend(finished(column, text, look));
            continue;
        }
        match open.as_mut() {
            Some((_, text, current)) if same_look(current, &look) => text.push(cell.character),
            _ => {
                close(&mut open, &mut runs);
                open = Some((column, cell.character.to_string(), look));
            }
        }
    }
    close(&mut open, &mut runs);
    runs
}

#[derive(Debug, Default)]
pub struct MoveFilter {
    last: Option<(ViewPoint, Side)>,
}

impl MoveFilter {
    pub fn changed(&mut self, point: ViewPoint, side: Side) -> bool {
        if self.last == Some((point, side)) {
            return false;
        }
        self.last = Some((point, side));
        true
    }

    pub fn reset(&mut self) {
        self.last = None;
    }
}

#[derive(Debug, Default)]
pub struct Grab {
    held: Option<MouseButton>,
    last: Option<(ViewPoint, Side)>,
}

impl Grab {
    pub fn begin(&mut self, button: MouseButton, point: ViewPoint, side: Side) {
        self.held = Some(button);
        self.last = Some((point, side));
    }

    pub fn moved(&mut self, point: ViewPoint, side: Side) {
        self.last = Some((point, side));
    }

    pub fn held(&self) -> Option<MouseButton> {
        self.held
    }

    pub fn finish(&mut self, button: MouseButton) -> Option<(ViewPoint, Side)> {
        if self.held != Some(button) {
            return None;
        }
        self.held = None;
        self.last
    }

    pub fn abandon(&mut self) -> Option<(MouseButton, ViewPoint, Side)> {
        let button = self.held.take()?;
        let (point, side) = self.last.unwrap_or((ViewPoint { row: 0, column: 0 }, Side::Left));
        Some((button, point, side))
    }
}

pub fn click_count(click: mouse::Click) -> u8 {
    match click.kind() {
        mouse::click::Kind::Single => 1,
        mouse::click::Kind::Double => 2,
        mouse::click::Kind::Triple => 3,
    }
}

pub fn term_button(button: mouse::Button) -> Option<MouseButton> {
    match button {
        mouse::Button::Left => Some(MouseButton::Left),
        mouse::Button::Middle => Some(MouseButton::Middle),
        mouse::Button::Right => Some(MouseButton::Right),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub point: ViewPoint,
    pub side: Side,
    pub clicks: u8,
    pub lines: i32,
}

struct State {
    metrics: Option<CellMetrics>,
    gate: ResizeGate,
    grab: Grab,
    last_click: Option<mouse::Click>,
    moves: MoveFilter,
}

pub struct TerminalGrid<'a, Message> {
    owner: u64,
    snapshot: Snapshot,
    palette: Palette,
    focused: bool,
    on_resize: Box<dyn Fn(TermSize) -> Message + 'a>,
    on_pointer: Box<dyn Fn(PointerEvent) -> Message + 'a>,
    link: Vec<(u16, std::ops::Range<u16>)>,
}

pub fn terminal_grid<'a, Message>(
    owner: u64, snapshot: Snapshot, palette: &Palette, focused: bool, on_resize: impl Fn(TermSize) -> Message + 'a,
    on_pointer: impl Fn(PointerEvent) -> Message + 'a,
) -> TerminalGrid<'a, Message> {
    TerminalGrid {
        owner,
        snapshot,
        palette: *palette,
        focused,
        on_resize: Box::new(on_resize),
        on_pointer: Box::new(on_pointer),
        link: Vec::new(),
    }
}

impl<Message> TerminalGrid<'_, Message> {
    pub fn link(mut self, link: Vec<(u16, std::ops::Range<u16>)>) -> Self {
        self.link = link;
        self
    }
}

fn monospace(bold: bool, italic: bool) -> Font {
    Font {
        weight: if bold { Weight::Bold } else { Weight::Normal },
        style: if italic { FontStyle::Italic } else { FontStyle::Normal },
        ..Font::MONOSPACE
    }
}

fn measure<Renderer>() -> CellMetrics
where
    Renderer: text::Renderer<Font = Font>,
{
    let sample = text::Text {
        content: "M",
        bounds: Size::INFINITE,
        size: Pixels(TEXT_SIZE),
        line_height: text::LineHeight::Absolute(Pixels(LINE_HEIGHT)),
        font: Font::MONOSPACE,
        align_x: text::Alignment::Default,
        align_y: Vertical::Top,
        shaping: text::Shaping::Basic,
        wrapping: text::Wrapping::None,
    };
    let width = Renderer::Paragraph::with_text(sample).min_bounds().width;
    let width = if width.is_finite() && width > 0.0 { width } else { TEXT_SIZE * FALLBACK_WIDTH_RATIO };
    CellMetrics { width, height: LINE_HEIGHT }
}

fn scroll_thumb(history: usize, display_offset: usize, rows: u16, track_height: f32) -> Option<(f32, f32)> {
    if history == 0 || display_offset == 0 || rows == 0 || !track_height.is_finite() || track_height <= 0.0 {
        return None;
    }
    let total = (history + usize::from(rows)) as f32;
    let height = (track_height * f32::from(rows) / total).max(SCROLL_THUMB_MIN).min(track_height);
    let progress = 1.0 - (display_offset.min(history) as f32 / history as f32);
    Some((progress * (track_height - height), height))
}

fn quad(bounds: Rectangle) -> Quad {
    Quad { bounds, border: Border::default(), shadow: Shadow::default(), snap: true }
}

impl<Message: Clone, Theme, Renderer> Widget<Message, Theme, Renderer> for TerminalGrid<'_, Message>
where
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font>,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State {
            metrics: None,
            gate: ResizeGate::default(),
            grab: Grab::default(),
            last_click: None,
            moves: MoveFilter::default(),
        })
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        if state.metrics.is_none() {
            state.metrics = Some(measure::<Renderer>());
        }
        layout::Node::new(limits.width(Length::Fill).height(Length::Fill).max())
    }

    fn update(
        &mut self, tree: &mut Tree, event: &iced::Event, layout: Layout<'_>, cursor: mouse::Cursor,
        _renderer: &Renderer, _clipboard: &mut dyn Clipboard, shell: &mut Shell<'_, Message>, _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let Some(metrics) = state.metrics else {
            return;
        };
        let bounds = layout.bounds();
        let grid = grid_size(bounds.size(), metrics);
        let over = cursor.is_over(bounds);
        let locate = |position: Point| {
            cell_at(Point::new(position.x - bounds.x, position.y - bounds.y), bounds.size(), metrics, grid)
        };
        let publish = |shell: &mut Shell<'_, Message>, kind, point, side, clicks, lines| {
            shell.publish((self.on_pointer)(PointerEvent { kind, point, side, clicks, lines }));
        };
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(button)) if over => {
                if let (Some(term), Some(position)) = (term_button(*button), cursor.position()) {
                    let click = mouse::Click::new(position, *button, state.last_click);
                    state.last_click = Some(click);
                    state.moves.reset();
                    let (point, side) = locate(position);
                    state.grab.begin(term, point, side);
                    publish(shell, PointerKind::Press(term), point, side, click_count(click), 0);
                    shell.capture_event();
                }
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if let Some(term) = term_button(*button) {
                    let held = state.grab.held() == Some(term);
                    if held || over {
                        let located = cursor.position().map(locate);
                        let last = state.grab.finish(term);
                        if let Some((point, side)) = located.or(last) {
                            state.moves.reset();
                            publish(shell, PointerKind::Release(term), point, side, 0, 0);
                            shell.capture_event();
                        }
                    }
                }
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let dragging = state.grab.held().is_some();
                if over || dragging {
                    let (point, side) = locate(*position);
                    if dragging {
                        state.grab.moved(point, side);
                    }
                    if state.moves.changed(point, side) {
                        publish(shell, PointerKind::Move { dragging }, point, side, 0, 0);
                    }
                }
            }
            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) if over => {
                let lines = wheel_lines(*delta, metrics.height);
                if lines != 0 {
                    let (point, side) = cursor.position().map_or((ViewPoint { row: 0, column: 0 }, Side::Left), locate);
                    publish(shell, PointerKind::Wheel, point, side, 0, lines);
                    shell.capture_event();
                }
            }
            iced::Event::Window(iced::window::Event::Unfocused) => {
                if let Some((button, point, side)) = state.grab.abandon() {
                    state.moves.reset();
                    publish(shell, PointerKind::Release(button), point, side, 0, 0);
                }
            }
            _ => {}
        }
        if let Some(size) = state.gate.changed_for(self.owner, grid) {
            shell.publish((self.on_resize)(size));
        }
    }

    fn draw(
        &self, tree: &Tree, renderer: &mut Renderer, _theme: &Theme, _style: &renderer::Style, layout: Layout<'_>,
        _cursor: mouse::Cursor, _viewport: &Rectangle,
    ) {
        let Some(metrics) = tree.state.downcast_ref::<State>().metrics else {
            return;
        };
        let bounds = layout.bounds();
        let columns = usize::from(self.snapshot.size.columns);
        let at = |column: u16, row: u16| {
            Point::new(bounds.x + f32::from(column) * metrics.width, bounds.y + f32::from(row) * metrics.height)
        };
        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(quad(bounds), self.palette.bg);
            let rows = self.snapshot.cells.chunks(columns).take(usize::from(self.snapshot.size.rows));
            for (row, cells) in rows.clone().enumerate() {
                for span in row_backgrounds(cells, &self.palette) {
                    let origin = at(span.column, row as u16);
                    let area = Rectangle::new(origin, Size::new(f32::from(span.width) * metrics.width, metrics.height));
                    renderer.fill_quad(quad(area), span.color);
                }
            }
            let cursor = self.snapshot.cursor;
            let cursor_cell = Rectangle::new(at(cursor.column, cursor.row), Size::new(metrics.width, metrics.height));
            if cursor.visible && self.focused && cursor.shape == CursorShape::Block {
                renderer.fill_quad(quad(cursor_cell), self.palette.accent.scale_alpha(CURSOR_FILL_ALPHA));
            }
            for (row, cells) in rows.enumerate() {
                let row = row as u16;
                for run in row_runs(cells, &self.palette) {
                    let origin = at(run.column, row);
                    let characters = run.text.chars().count() as f32;
                    let width = characters * metrics.width;
                    let content = text::Text {
                        content: run.text.clone(),
                        bounds: Size::new(width + metrics.width, metrics.height),
                        size: Pixels(TEXT_SIZE),
                        line_height: text::LineHeight::Absolute(Pixels(LINE_HEIGHT)),
                        font: monospace(run.bold, run.italic),
                        align_x: text::Alignment::Default,
                        align_y: Vertical::Top,
                        shaping: if run.text.is_ascii() { text::Shaping::Basic } else { text::Shaping::Advanced },
                        wrapping: text::Wrapping::None,
                    };
                    renderer.fill_text(content, origin, run.color, bounds);
                    if run.underline {
                        let line = Rectangle::new(
                            Point::new(origin.x, origin.y + metrics.height - DECORATION_THICKNESS),
                            Size::new(width, DECORATION_THICKNESS),
                        );
                        renderer.fill_quad(quad(line), run.color);
                    }
                    if run.strikeout {
                        let line = Rectangle::new(
                            Point::new(origin.x, origin.y + metrics.height / 2.0),
                            Size::new(width, DECORATION_THICKNESS),
                        );
                        renderer.fill_quad(quad(line), run.color);
                    }
                }
            }
            for (row, columns) in &self.link {
                let origin = at(columns.start, *row);
                let line = Rectangle::new(
                    Point::new(origin.x, origin.y + metrics.height - DECORATION_THICKNESS),
                    Size::new(f32::from(columns.end - columns.start) * metrics.width, DECORATION_THICKNESS),
                );
                renderer.fill_quad(quad(line), self.palette.accent);
            }
            if cursor.visible {
                let accent = self.palette.accent;
                match (cursor.shape, self.focused) {
                    (CursorShape::Block, false) => {
                        let mut outline = quad(cursor_cell);
                        outline.border = Border { color: accent, width: DECORATION_THICKNESS, radius: 0.0.into() };
                        renderer.fill_quad(outline, Color::TRANSPARENT);
                    }
                    (CursorShape::Block, true) => {}
                    (CursorShape::Underline, _) => {
                        let line = Rectangle::new(
                            Point::new(cursor_cell.x, cursor_cell.y + metrics.height - CURSOR_BAR_WIDTH),
                            Size::new(metrics.width, CURSOR_BAR_WIDTH),
                        );
                        renderer.fill_quad(quad(line), accent);
                    }
                    (CursorShape::Bar, _) => {
                        let line = Rectangle::new(cursor_cell.position(), Size::new(CURSOR_BAR_WIDTH, metrics.height));
                        renderer.fill_quad(quad(line), accent);
                    }
                }
            }
            let thumb = scroll_thumb(
                self.snapshot.history,
                self.snapshot.display_offset,
                self.snapshot.size.rows,
                bounds.height,
            );
            if let Some((top, height)) = thumb {
                let thumb_bounds = Rectangle::new(
                    Point::new(bounds.x + bounds.width - SCROLL_THUMB_WIDTH, bounds.y + top),
                    Size::new(SCROLL_THUMB_WIDTH, height),
                );
                let mut thumb_quad = quad(thumb_bounds);
                thumb_quad.border = Border { radius: (SCROLL_THUMB_WIDTH / 2.0).into(), ..Border::default() };
                renderer.fill_quad(thumb_quad, self.palette.line_strong);
            }
        });
    }
}

impl<'a, Message, Theme, Renderer> From<TerminalGrid<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font> + 'a,
{
    fn from(grid: TerminalGrid<'a, Message>) -> Self {
        Element::new(grid)
    }
}

#[cfg(test)]
mod tests;
