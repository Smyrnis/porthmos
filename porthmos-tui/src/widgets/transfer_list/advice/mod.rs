const ADVICE: &[(&[&str], &str)] = &[
    (&["permission denied"], "check the file's permissions or ask the server admin for access"),
    (&["no space left", "quota"], "free some space on the destination and retry"),
    (&["no such file", "not found"], "the file was moved or deleted; refresh the listing"),
    (
        &["timed out", "connection reset", "broken pipe", "connection refused", "disconnected"],
        "check the connection, then retry",
    ),
];

pub fn advice_for(message: &str) -> Option<&'static str> {
    let lowered = message.to_lowercase();
    ADVICE.iter().find(|(needles, _)| needles.iter().any(|needle| lowered.contains(needle))).map(|(_, advice)| *advice)
}

#[cfg(test)]
mod tests;
