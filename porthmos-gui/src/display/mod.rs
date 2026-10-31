pub const NO_DISPLAY_MESSAGE: &str =
    "porthmos-gui: no display found (DISPLAY/WAYLAND_DISPLAY not set); use porthmos for the terminal version";

fn missing_in(lookup: impl Fn(&str) -> Option<String>) -> bool {
    let set = |name: &str| lookup(name).is_some_and(|value| !value.is_empty());
    !set("DISPLAY") && !set("WAYLAND_DISPLAY")
}

pub fn is_missing() -> bool {
    cfg!(all(unix, not(target_os = "macos"))) && missing_in(|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests;
