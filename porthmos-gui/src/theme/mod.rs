use iced::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(text: &str) -> Option<Rgb> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&digits[range], 16).ok();
        Some(Rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?))
    }

    fn color(self) -> Color {
        Color::from_rgb8(self.0, self.1, self.2)
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb(red, green, blue)
}

const ACCENT: Rgb = rgb(0x9f, 0xb3, 0xc8);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: Color,
    pub fg_bright: Color,
    pub fg_focus: Color,
    pub fg: Color,
    pub fg_muted: Color,
    pub fg_dim: Color,
    pub line: Color,
    pub line_strong: Color,
    pub row_focus: Color,
    pub row_focus_inactive: Color,
    pub surface: Color,
    pub accent: Color,
    pub ok: Color,
    pub ok_dim: Color,
    pub err: Color,
    pub idle: Color,
    pub paused: Color,
    pub inactive_file: Color,
    pub inactive_dir: Color,
}

impl Palette {
    pub fn new(settings: &ThemeSettings) -> Palette {
        Palette {
            bg: rgb(0x00, 0x00, 0x00).color(),
            fg_bright: rgb(0xed, 0xed, 0xed).color(),
            fg_focus: rgb(0xff, 0xff, 0xff).color(),
            fg: rgb(0xb3, 0xb3, 0xb3).color(),
            fg_muted: rgb(0x8a, 0x8a, 0x8a).color(),
            fg_dim: rgb(0x6e, 0x6e, 0x6e).color(),
            line: rgb(0x1c, 0x1c, 0x1c).color(),
            line_strong: rgb(0x26, 0x26, 0x26).color(),
            row_focus: rgb(0x16, 0x16, 0x16).color(),
            row_focus_inactive: rgb(0x0b, 0x0b, 0x0b).color(),
            surface: rgb(0x0a, 0x0a, 0x0a).color(),
            accent: settings.accent.unwrap_or(ACCENT).color(),
            ok: rgb(0x8f, 0xb3, 0x8a).color(),
            ok_dim: rgb(0x4f, 0x6b, 0x4c).color(),
            err: rgb(0xc9, 0x8b, 0x84).color(),
            idle: rgb(0x33, 0x33, 0x33).color(),
            paused: rgb(0x5a, 0x5a, 0x5a).color(),
            inactive_file: rgb(0x7a, 0x7a, 0x7a).color(),
            inactive_dir: rgb(0xa0, 0xa0, 0xa0).color(),
        }
    }

    pub fn iced_theme(&self) -> iced::Theme {
        iced::Theme::custom(
            "Porthmos".to_string(),
            iced::theme::Palette {
                background: self.bg,
                text: self.fg,
                primary: self.accent,
                success: self.ok,
                warning: self.paused,
                danger: self.err,
            },
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThemeSettings {
    pub accent: Option<Rgb>,
}

impl ThemeSettings {
    pub fn from_frontend(frontend: &toml::Table) -> (Self, Vec<String>) {
        let mut settings = Self::default();
        let mut warnings = Vec::new();
        let Some(gui) = frontend.get("gui") else {
            return (settings, warnings);
        };
        let Some(gui) = gui.as_table() else {
            warnings.push("[gui] must be a table; using the default settings".to_string());
            return (settings, warnings);
        };
        let Some(theme) = gui.get("theme") else {
            return (settings, warnings);
        };
        let Some(theme) = theme.as_table() else {
            warnings.push("[gui.theme] must be a table; using the default theme".to_string());
            return (settings, warnings);
        };
        for (key, value) in theme {
            match (key.as_str(), value.as_str()) {
                ("accent", Some(text)) => match Rgb::parse(text) {
                    Some(color) => settings.accent = Some(color),
                    None => warnings
                        .push(format!("invalid gui theme accent \"{text}\": expected a colour like \"#9fb3c8\"")),
                },
                ("accent", None) => warnings.push("gui theme accent must be a string; using the default".to_string()),
                _ => warnings.push(format!("unknown gui theme setting \"{key}\"")),
            }
        }
        (settings, warnings)
    }
}

#[cfg(test)]
mod tests;
