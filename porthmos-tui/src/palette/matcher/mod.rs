const MATCH: i32 = 10;
const WORD_START: i32 = 8;
const CAMEL_STEP: i32 = 4;
const CONSECUTIVE: i32 = 6;
const GAP: i32 = 1;
const LEADING_GAP_CAP: usize = 6;
const LENGTH_DIVISOR: i32 = 8;
const MAX_CELLS: usize = 1 << 22;
const UNREACHABLE: i32 = i32::MIN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub score: i32,
    pub indices: Vec<usize>,
}

fn fold(character: char) -> char {
    character.to_lowercase().next().unwrap_or(character)
}

fn leftmost(wanted: &[char], folded: &[char]) -> Option<Vec<usize>> {
    let mut indices = Vec::with_capacity(wanted.len());
    let mut position = 0;
    for target in wanted {
        let found = folded[position..].iter().position(|character| character == target)?;
        indices.push(position + found);
        position += found + 1;
    }
    Some(indices)
}

fn word_bonus(original: &[char], index: usize) -> i32 {
    let Some(previous) = index.checked_sub(1).map(|before| original[before]) else {
        return WORD_START;
    };
    if matches!(previous, ' ' | '/' | '-' | '_' | '.' | ':') {
        WORD_START
    } else if previous.is_lowercase() && original[index].is_uppercase() {
        CAMEL_STEP
    } else {
        0
    }
}

fn best_alignment(wanted: &[char], original: &[char], folded: &[char]) -> Match {
    let (rows, columns) = (wanted.len(), folded.len());
    let bonus: Vec<i32> = (0..columns).map(|index| word_bonus(original, index)).collect();
    let mut previous = vec![UNREACHABLE; columns];
    let mut current = vec![UNREACHABLE; columns];
    let mut from = vec![0u32; rows * columns];

    for (index, character) in folded.iter().enumerate() {
        if *character == wanted[0] {
            previous[index] = MATCH + bonus[index] - index.min(LEADING_GAP_CAP) as i32;
        }
    }
    for row in 1..rows {
        current.fill(UNREACHABLE);
        let mut base = UNREACHABLE;
        let mut base_from = 0;
        for column in 0..columns {
            if column >= 2 && previous[column - 2] != UNREACHABLE {
                let value = previous[column - 2] + (column - 2) as i32 * GAP;
                if base == UNREACHABLE || value > base {
                    base = value;
                    base_from = column - 2;
                }
            }
            if folded[column] != wanted[row] {
                continue;
            }
            let mut best = UNREACHABLE;
            let mut pick = 0;
            if column >= 1 && previous[column - 1] != UNREACHABLE {
                best = previous[column - 1] + CONSECUTIVE;
                pick = column - 1;
            }
            if base != UNREACHABLE {
                let gapped = base - (column as i32 - 1) * GAP;
                if best == UNREACHABLE || gapped > best {
                    best = gapped;
                    pick = base_from;
                }
            }
            if best != UNREACHABLE {
                current[column] = best + MATCH + bonus[column];
                from[row * columns + column] = pick as u32;
            }
        }
        std::mem::swap(&mut previous, &mut current);
    }

    let end = (0..columns)
        .filter(|column| previous[*column] != UNREACHABLE)
        .max_by(|left, right| previous[*left].cmp(&previous[*right]).then(right.cmp(left)))
        .unwrap_or(columns - 1);
    let score = previous[end] - columns as i32 / LENGTH_DIVISOR;
    let mut indices = Vec::with_capacity(rows);
    let mut column = end;
    for row in (0..rows).rev() {
        indices.push(column);
        if row > 0 {
            column = from[row * columns + column] as usize;
        }
    }
    indices.reverse();
    Match { score, indices }
}

pub fn fuzzy_match(query: &str, candidate: &str) -> Option<Match> {
    let wanted: Vec<char> = query.chars().map(fold).collect();
    if wanted.is_empty() {
        return Some(Match { score: 0, indices: Vec::new() });
    }
    let original: Vec<char> = candidate.chars().collect();
    let folded: Vec<char> = original.iter().copied().map(fold).collect();
    if wanted.len() > folded.len() {
        return None;
    }
    let greedy = leftmost(&wanted, &folded)?;
    if wanted.len().saturating_mul(folded.len()) > MAX_CELLS {
        return Some(Match {
            score: i32::try_from(wanted.len()).unwrap_or(i32::MAX).saturating_mul(MATCH),
            indices: greedy,
        });
    }
    Some(best_alignment(&wanted, &original, &folded))
}

#[cfg(test)]
mod tests;
