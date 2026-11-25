use std::ops::Range;

use porthmos_term::{Cell, CellWidth, Snapshot, Style, ViewPoint, is_openable, url_at};

pub type Segment = (u16, Range<u16>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub target: String,
    pub segments: Vec<Segment>,
    pub from_hyperlink: bool,
    pub label: String,
}

fn visible_cells(snapshot: &Snapshot, row: u16) -> Vec<(u16, &Cell)> {
    let columns = usize::from(snapshot.size.columns);
    let start = usize::from(row) * columns;
    snapshot.cells[start..start + columns]
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.width != CellWidth::Spacer)
        .map(|(column, cell)| (column as u16, cell))
        .collect()
}

fn span_end(cells: &[(u16, &Cell)], index: usize, columns: u16) -> u16 {
    cells.get(index + 1).map_or(columns, |(column, _)| *column)
}

fn is_hidden(cell: &Cell) -> bool {
    cell.style.contains(Style::HIDDEN)
}

fn hyperlink_at(cells: &[(u16, &Cell)], index: usize, row: u16, columns: u16) -> Option<Link> {
    let target = cells[index].1.hyperlink.clone()?;
    let same = |cell: &Cell| cell.hyperlink.as_deref() == Some(&*target) && !is_hidden(cell);
    let mut first = index;
    while first > 0 && same(cells[first - 1].1) {
        first -= 1;
    }
    let mut last = index;
    while last + 1 < cells.len() && same(cells[last + 1].1) {
        last += 1;
    }
    let label: String = cells[first..=last].iter().map(|(_, cell)| cell.character).collect();
    Some(Link {
        target: target.to_string(),
        segments: vec![(row, cells[first].0..span_end(cells, last, columns))],
        from_hyperlink: true,
        label,
    })
}

fn continues_below(snapshot: &Snapshot, row: u16) -> bool {
    snapshot.wrapped.get(usize::from(row)).copied().unwrap_or(false)
}

fn plain_url_at(snapshot: &Snapshot, point: ViewPoint) -> Option<Link> {
    let mut first_row = point.row;
    while first_row > 0 && continues_below(snapshot, first_row - 1) {
        first_row -= 1;
    }
    let mut last_row = point.row;
    while last_row + 1 < snapshot.size.rows && continues_below(snapshot, last_row) {
        last_row += 1;
    }
    let columns = snapshot.size.columns;
    let mut text = String::new();
    let mut places: Vec<(u16, u16, u16)> = Vec::new();
    let mut clicked = None;
    for row in first_row..=last_row {
        let cells = visible_cells(snapshot, row);
        for (index, (column, cell)) in cells.iter().enumerate() {
            if row == point.row && *column <= point.column {
                clicked = Some(places.len());
            }
            text.push(if is_hidden(cell) { ' ' } else { cell.character });
            places.push((row, *column, span_end(&cells, index, columns)));
        }
    }
    let range = url_at(&text, clicked?)?;
    let target: String = text.chars().skip(range.start).take(range.len()).collect();
    let mut segments: Vec<Segment> = Vec::new();
    for (row, start, end) in &places[range] {
        match segments.last_mut() {
            Some((last_row, columns)) if last_row == row => columns.end = *end,
            _ => segments.push((*row, *start..*end)),
        }
    }
    Some(Link { label: target.clone(), target, segments, from_hyperlink: false })
}

pub fn link_at(snapshot: &Snapshot, point: ViewPoint) -> Option<Link> {
    if point.row >= snapshot.size.rows || point.column >= snapshot.size.columns {
        return None;
    }
    let cells = visible_cells(snapshot, point.row);
    let index = cells.iter().rposition(|(column, _)| *column <= point.column)?;
    if is_hidden(cells[index].1) {
        return None;
    }
    hyperlink_at(&cells, index, point.row, snapshot.size.columns).or_else(|| plain_url_at(snapshot, point))
}

pub fn is_clickable(link: &Link) -> bool {
    let plain_mailto = !link.from_hyperlink && link.target.to_ascii_lowercase().starts_with("mailto:");
    !plain_mailto && is_openable(&link.target)
}

pub fn needs_confirmation(link: &Link) -> bool {
    link.from_hyperlink && link.label != link.target
}

#[cfg(test)]
mod tests;
