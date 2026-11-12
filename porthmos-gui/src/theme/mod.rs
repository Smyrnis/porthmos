use iced::Color;
use porthmos_core::theme::ColorScheme;

pub use porthmos_core::theme::Rgb;

fn color_of(rgb: Rgb) -> Color {
    Color::from_rgb8(rgb.0, rgb.1, rgb.2)
}

pub fn mix(from: Color, to: Color, amount: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
}

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
    pub light: bool,
}

impl Palette {
    pub fn new(settings: &ThemeSettings) -> Palette {
        let scheme = settings.scheme.with_accent(settings.accent);
        Palette {
            bg: color_of(scheme.bg),
            fg_bright: color_of(scheme.fg_bright),
            fg_focus: color_of(scheme.fg_focus),
            fg: color_of(scheme.fg),
            fg_muted: color_of(scheme.fg_muted),
            fg_dim: color_of(scheme.fg_dim),
            line: color_of(scheme.line),
            line_strong: color_of(scheme.line_strong),
            row_focus: color_of(scheme.row_focus),
            row_focus_inactive: color_of(scheme.row_focus_inactive),
            surface: color_of(scheme.surface),
            accent: color_of(scheme.accent),
            ok: color_of(scheme.ok),
            ok_dim: color_of(scheme.ok_dim),
            err: color_of(scheme.err),
            idle: color_of(scheme.idle),
            paused: color_of(scheme.paused),
            inactive_file: color_of(scheme.inactive_file),
            inactive_dir: color_of(scheme.inactive_dir),
            light: scheme.is_light(),
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
    pub scheme: ColorScheme,
}

impl ThemeSettings {
    pub fn with_scheme(self, scheme: ColorScheme) -> Self {
        Self { scheme, accent: None }
    }

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
