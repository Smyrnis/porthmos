use std::cmp::Ordering;

use porthmos_vfs::Entry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSpec {
    pub key: SortKey,
    pub order: SortOrder,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self { key: SortKey::Name, order: SortOrder::Ascending }
    }
}

impl SortSpec {
    pub fn cycled(self) -> SortSpec {
        match (self.key, self.order) {
            (SortKey::Name, SortOrder::Ascending) => SortSpec { key: SortKey::Name, order: SortOrder::Descending },
            (SortKey::Name, SortOrder::Descending) => SortSpec { key: SortKey::Size, order: SortOrder::Ascending },
            (SortKey::Size, SortOrder::Ascending) => SortSpec { key: SortKey::Size, order: SortOrder::Descending },
            (SortKey::Size, SortOrder::Descending) => SortSpec { key: SortKey::Name, order: SortOrder::Ascending },
        }
    }
}

pub fn sort_entries(entries: &mut [Entry], spec: SortSpec) {
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| compare_by(a, b, spec)));
}

fn compare_by(a: &Entry, b: &Entry, spec: SortSpec) -> Ordering {
    let primary = match spec.key {
        SortKey::Name => natural_cmp(&a.name, &b.name),
        SortKey::Size => a.size.cmp(&b.size),
    };
    let primary = match spec.order {
        SortOrder::Ascending => primary,
        SortOrder::Descending => primary.reverse(),
    };
    primary.then_with(|| natural_cmp(&a.name, &b.name)).then_with(|| a.name.cmp(&b.name))
}

fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(digit) = chars.next_if(char::is_ascii_digit) {
        digits.push(digit);
    }
    digits
}

fn compare_numbers(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut left, mut right) = (a.chars().peekable(), b.chars().peekable());
    let mut zeros_tiebreak = Ordering::Equal;
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return zeros_tiebreak,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (run_left, run_right) = (take_digits(&mut left), take_digits(&mut right));
                match compare_numbers(&run_left, &run_right) {
                    Ordering::Equal => {
                        if zeros_tiebreak == Ordering::Equal {
                            zeros_tiebreak = run_left.len().cmp(&run_right.len());
                        }
                    }
                    other => return other,
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                left.next();
                right.next();
            }
        }
    }
}

#[cfg(test)]
mod tests;
