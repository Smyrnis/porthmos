use porthmos_core::theme::ColorScheme;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Indexed,
}

impl ColorDepth {
    pub fn detect(colorterm: Option<&str>, windows_terminal_session: Option<&str>) -> Self {
        let colorterm_says_true_colour =
            matches!(colorterm.map(str::to_ascii_lowercase).as_deref(), Some("truecolor" | "24bit"));
        let in_windows_terminal = windows_terminal_session.is_some_and(|session| !session.is_empty());
        if colorterm_says_true_colour || in_windows_terminal { ColorDepth::TrueColor } else { ColorDepth::Indexed }
    }

    pub fn from_environment() -> Self {
        Self::detect(std::env::var("COLORTERM").ok().as_deref(), std::env::var("WT_SESSION").ok().as_deref())
    }
}

pub use porthmos_core::theme::Rgb;

const HAND_PICKED_INDEXES: [(Rgb, u8); 1] = [(Rgb(0x9f, 0xb3, 0xc8), 110)];

fn indexed_of(rgb: Rgb) -> u8 {
    HAND_PICKED_INDEXES
        .iter()
        .find(|(known, _)| *known == rgb)
        .map_or_else(|| rgb.nearest_indexed(), |(_, index)| *index)
}

fn color_of(depth: ColorDepth, rgb: Rgb) -> Color {
    match depth {
        ColorDepth::TrueColor => Color::Rgb(rgb.0, rgb.1, rgb.2),
        ColorDepth::Indexed => Color::Indexed(indexed_of(rgb)),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeSettings {
    pub accent: Option<Rgb>,
    pub terminal_background: bool,
    pub scheme: ColorScheme,
}

impl ThemeSettings {
    pub fn with_scheme(self, scheme: ColorScheme) -> Self {
        Self { scheme, accent: None, ..self }
    }

    pub fn from_frontend(frontend: &toml::Table) -> (Self, Vec<String>) {
        let mut settings = Self::default();
        let mut warnings = Vec::new();
        let Some(entry) = frontend.get("theme") else {
            return (settings, warnings);
        };
        let Some(table) = entry.as_table() else {
            warnings.push("[theme] must be a table; using the default theme".to_string());
            return (settings, warnings);
        };

        for (key, value) in table {
            match (key.as_str(), value.as_str()) {
                ("accent", Some(text)) => match Rgb::parse(text) {
                    Some(rgb) => settings.accent = Some(rgb),
                    None => {
                        warnings.push(format!("invalid theme accent \"{text}\": expected a colour like \"#9fb3c8\""))
                    }
                },
                ("background", Some("black")) => settings.terminal_background = false,
                ("background", Some("terminal")) => settings.terminal_background = true,
                ("background", Some(text)) => {
                    warnings.push(format!("invalid theme background \"{text}\": expected \"black\" or \"terminal\""));
                }
                ("accent" | "background", None) => {
                    warnings.push(format!("theme {key} must be a string; using the default"));
                }
                _ => warnings.push(format!("unknown theme setting \"{key}\"")),
            }
        }
        (settings, warnings)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
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

impl Theme {
    pub fn new(depth: ColorDepth, settings: &ThemeSettings) -> Self {
        let scheme = settings.scheme.with_accent(settings.accent);
        let pick = |rgb: Rgb| color_of(depth, rgb);
        Self {
            bg: if settings.terminal_background { Color::Reset } else { pick(scheme.bg) },
            fg_bright: pick(scheme.fg_bright),
            fg_focus: pick(scheme.fg_focus),
            fg: pick(scheme.fg),
            fg_muted: pick(scheme.fg_muted),
            fg_dim: pick(scheme.fg_dim),
            line: pick(scheme.line),
            line_strong: pick(scheme.line_strong),
            row_focus: pick(scheme.row_focus),
            row_focus_inactive: pick(scheme.row_focus_inactive),
            surface: pick(scheme.surface),
            accent: pick(scheme.accent),
            ok: pick(scheme.ok),
            ok_dim: pick(scheme.ok_dim),
            err: pick(scheme.err),
            idle: pick(scheme.idle),
            paused: pick(scheme.paused),
            inactive_file: pick(scheme.inactive_file),
            inactive_dir: pick(scheme.inactive_dir),
        }
    }

    pub fn idle_text(&self) -> Style {
        Style::default().fg(self.idle)
    }

    pub fn paused_text(&self) -> Style {
        Style::default().fg(self.paused)
    }

    pub fn ok_dim_text(&self) -> Style {
        Style::default().fg(self.ok_dim)
    }

    pub fn line_strong_text(&self) -> Style {
        Style::default().fg(self.line_strong)
    }

    pub fn directory_name(&self) -> Style {
        Style::default().fg(self.fg_bright).add_modifier(Modifier::BOLD)
    }

    pub fn inactive_file_name(&self) -> Style {
        Style::default().fg(self.inactive_file)
    }

    pub fn inactive_dir_name(&self) -> Style {
        Style::default().fg(self.inactive_dir).add_modifier(Modifier::BOLD)
    }

    pub fn text(&self) -> Style {
        Style::default().fg(self.fg)
    }

    pub fn bright(&self) -> Style {
        Style::default().fg(self.fg_bright)
    }

    pub fn focused_name(&self) -> Style {
        Style::default().fg(self.fg_focus).add_modifier(Modifier::BOLD)
    }

    pub fn muted(&self) -> Style {
        Style::default().fg(self.fg_muted)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.fg_dim)
    }

    pub fn accent_text(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn ok_text(&self) -> Style {
        Style::default().fg(self.ok)
    }

    pub fn err_text(&self) -> Style {
        Style::default().fg(self.err)
    }

    pub fn warning_text(&self) -> Style {
        Style::default().fg(self.fg_bright)
    }

    pub fn hairline(&self) -> Style {
        Style::default().fg(self.line)
    }

    pub fn popup_border(&self) -> Style {
        Style::default().fg(self.line_strong)
    }

    pub fn backdrop(&self) -> Style {
        Style::default().fg(self.line_strong)
    }

    pub fn focus_row(&self) -> Style {
        Style::default().bg(self.row_focus)
    }

    pub fn focus_row_inactive(&self) -> Style {
        Style::default().bg(self.row_focus_inactive)
    }

    pub fn toast_surface(&self) -> Style {
        Style::default().bg(self.surface)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(ColorDepth::TrueColor, &ThemeSettings::default())
    }
}

#[cfg(test)]
mod tests;
