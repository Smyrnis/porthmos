use porthmos_core::TerminalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Right,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Layout {
    Leaf(TerminalId),
    Split { direction: Direction, first: Box<Layout>, second: Box<Layout> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Missing,
    Last,
    Removed { focus: TerminalId },
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

fn overlap(low_a: f64, high_a: f64, low_b: f64, high_b: f64) -> f64 {
    (high_a.min(high_b) - low_a.max(low_b)).max(0.0)
}

impl Layout {
    pub fn leaves(&self) -> Vec<TerminalId> {
        match self {
            Layout::Leaf(id) => vec![*id],
            Layout::Split { first, second, .. } => {
                let mut leaves = first.leaves();
                leaves.extend(second.leaves());
                leaves
            }
        }
    }

    pub fn contains(&self, id: TerminalId) -> bool {
        match self {
            Layout::Leaf(leaf) => *leaf == id,
            Layout::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    pub fn split(&mut self, target: TerminalId, new: TerminalId, direction: Direction) -> bool {
        match self {
            Layout::Leaf(leaf) if *leaf == target => {
                *self = Layout::Split {
                    direction,
                    first: Box::new(Layout::Leaf(target)),
                    second: Box::new(Layout::Leaf(new)),
                };
                true
            }
            Layout::Leaf(_) => false,
            Layout::Split { first, second, .. } => {
                first.split(target, new, direction) || second.split(target, new, direction)
            }
        }
    }

    pub fn remove(&mut self, id: TerminalId) -> Removal {
        let Layout::Split { first, second, .. } = self else {
            return if *self == Layout::Leaf(id) { Removal::Last } else { Removal::Missing };
        };
        let removed_first = **first == Layout::Leaf(id);
        if !removed_first && **second != Layout::Leaf(id) {
            return match first.remove(id) {
                Removal::Missing => second.remove(id),
                other => other,
            };
        }
        let survivor = std::mem::replace(if removed_first { &mut **second } else { &mut **first }, Layout::Leaf(id));
        let leaves = survivor.leaves();
        let focus = if removed_first { leaves[0] } else { leaves[leaves.len() - 1] };
        *self = survivor;
        Removal::Removed { focus }
    }

    fn rects(&self, area: Rect, found: &mut Vec<(TerminalId, Rect)>) {
        match self {
            Layout::Leaf(id) => found.push((*id, area)),
            Layout::Split { direction, first, second } => {
                let (one, two) = match direction {
                    Direction::Right => {
                        let middle = (area.left + area.right) / 2.0;
                        (Rect { right: middle, ..area }, Rect { left: middle, ..area })
                    }
                    Direction::Down => {
                        let middle = (area.top + area.bottom) / 2.0;
                        (Rect { bottom: middle, ..area }, Rect { top: middle, ..area })
                    }
                };
                first.rects(one, found);
                second.rects(two, found);
            }
        }
    }

    pub fn neighbour(&self, from: TerminalId, toward: Toward) -> Option<TerminalId> {
        let mut rects = Vec::new();
        self.rects(Rect { left: 0.0, top: 0.0, right: 1.0, bottom: 1.0 }, &mut rects);
        let (_, origin) = *rects.iter().find(|(id, _)| *id == from)?;
        let mut best: Option<(TerminalId, f64)> = None;
        for (id, rect) in rects.iter().filter(|(id, _)| *id != from) {
            let (touching, shared) = match toward {
                Toward::Left => (rect.right == origin.left, overlap(origin.top, origin.bottom, rect.top, rect.bottom)),
                Toward::Right => (rect.left == origin.right, overlap(origin.top, origin.bottom, rect.top, rect.bottom)),
                Toward::Up => (rect.bottom == origin.top, overlap(origin.left, origin.right, rect.left, rect.right)),
                Toward::Down => (rect.top == origin.bottom, overlap(origin.left, origin.right, rect.left, rect.right)),
            };
            if touching && shared > 0.0 && best.is_none_or(|(_, most)| shared > most) {
                best = Some((*id, shared));
            }
        }
        best.map(|(id, _)| id)
    }
}

#[cfg(test)]
mod tests;
