use porthmos_core::i18n;

const ADVICE: &[(&[&str], &str)] = &[
    (&["permission denied"], "advice.permissions"),
    (&["no space left", "quota"], "advice.space"),
    (&["no such file", "not found"], "advice.moved"),
    (&["timed out", "connection reset", "broken pipe", "connection refused", "disconnected"], "advice.connection"),
];

pub fn advice_for(message: &str) -> Option<String> {
    let lowered = message.to_lowercase();
    ADVICE
        .iter()
        .find(|(needles, _)| needles.iter().any(|needle| lowered.contains(needle)))
        .map(|(_, advice)| i18n::text(advice))
}

#[cfg(test)]
mod tests;
