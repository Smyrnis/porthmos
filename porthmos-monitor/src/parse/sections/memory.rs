use std::collections::HashMap;

use crate::{
    Memory,
    parse::split::{Name, Split},
};

const KIBIBYTE: u64 = 1024;

pub(crate) fn memory(split: &Split) -> Option<Memory> {
    let lines = split.lines(Name::Meminfo)?;
    let mut values: HashMap<String, u64> = HashMap::new();
    for line in lines {
        let text = String::from_utf8_lossy(line);
        let Some((key, rest)) = text.split_once(':') else { continue };
        let Some(number) = rest.split_whitespace().next().and_then(|number| number.parse::<u64>().ok()) else {
            continue;
        };
        values.entry(key.trim().to_string()).or_insert(number.saturating_mul(KIBIBYTE));
    }
    let get = |key: &str| values.get(key).copied();
    let total = get("MemTotal")?;
    let available = get("MemAvailable").or_else(|| {
        let free = get("MemFree")?;
        Some(free.saturating_add(get("Buffers").unwrap_or(0)).saturating_add(get("Cached").unwrap_or(0)))
    })?;
    Some(Memory {
        total,
        available: available.min(total),
        swap_total: get("SwapTotal").unwrap_or(0),
        swap_free: get("SwapFree").unwrap_or(0).min(get("SwapTotal").unwrap_or(0)),
    })
}
