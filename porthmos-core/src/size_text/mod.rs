const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];

pub fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while (value >= 1024.0 || (value * 10.0).round() / 10.0 >= 1000.0) && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests;
