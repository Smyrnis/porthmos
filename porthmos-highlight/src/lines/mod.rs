pub fn lines_of(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines.into_iter().map(|line| line.strip_suffix('\r').unwrap_or(line)).collect()
}

#[cfg(test)]
mod tests;
