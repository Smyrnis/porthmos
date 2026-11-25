use alacritty_terminal::{
    event::EventListener,
    grid::Dimensions,
    index::{Column, Direction, Line, Point, Side},
    term::{Term, search::RegexSearch},
};

use crate::{GridPoint, Match, SearchDirection};

const REGEX_META: &str = "\\.+*?()|[]{}^$";

fn escape_literal(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for character in query.chars() {
        if REGEX_META.contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

pub(crate) fn to_point<T>(term: &Term<T>, point: GridPoint) -> Point {
    let line = point.line.clamp(term.topmost_line().0, term.bottommost_line().0);
    Point::new(Line(line), Column(usize::from(point.column).min(term.columns() - 1)))
}

fn to_grid_point(point: Point) -> GridPoint {
    GridPoint { line: point.line.0, column: point.column.0 as u16 }
}

pub(crate) fn find<T: EventListener>(
    term: &Term<T>, query: &str, from: GridPoint, direction: SearchDirection,
) -> Option<Match> {
    if query.is_empty() {
        return None;
    }
    let mut regex = RegexSearch::new(&escape_literal(query)).ok()?;
    let (direction, side) = match direction {
        SearchDirection::Forward => (Direction::Right, Side::Left),
        SearchDirection::Backward => (Direction::Left, Side::Right),
    };
    let found = term.search_next(&mut regex, to_point(term, from), direction, side, None)?;
    Some(Match { start: to_grid_point(*found.start()), end: to_grid_point(*found.end()) })
}

#[cfg(test)]
mod tests;
