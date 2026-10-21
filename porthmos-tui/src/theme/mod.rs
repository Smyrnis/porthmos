use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Indexed,
}

impl ColorDepth {
    pub fn detect(colorterm: Option<&str>) -> Self {
        match colorterm.map(str::to_ascii_lowercase).as_deref() {
            Some("truecolor" | "24bit") => ColorDepth::TrueColor,
            _ => ColorDepth::Indexed,
        }
    }

    pub fn from_environment() -> Self {
        Self::detect(std::env::var("COLORTERM").ok().as_deref())
    }
}

const CUBE_STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];

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

    pub fn color(self, depth: ColorDepth, indexed: u8) -> Color {
        match depth {
            ColorDepth::TrueColor => Color::Rgb(self.0, self.1, self.2),
            ColorDepth::Indexed => Color::Indexed(indexed),
        }
    }

    pub fn nearest_indexed(self) -> u8 {
        let Rgb(red, green, blue) = self;
        let level = |value: u8| -> u8 {
            (0..6u8).min_by_key(|i| (i32::from(CUBE_STEPS[usize::from(*i)]) - i32::from(value)).abs()).unwrap_or(0)
        };
        let cube = |r: u8, g: u8, b: u8| 16 + 36 * r + 6 * g + b;
        let cube_index = cube(level(red), level(green), level(blue));
        let cube_rgb = |index: u8| {
            let offset = index - 16;
            let step = |position: u8| i32::from(CUBE_STEPS[usize::from(position)]);
            (step(offset / 36), step(offset / 6 % 6), step(offset % 6))
        };
        let distance = |(r, g, b): (i32, i32, i32)| {
            (r - i32::from(red)).pow(2) + (g - i32::from(green)).pow(2) + (b - i32::from(blue)).pow(2)
        };

        let average = (i32::from(red) + i32::from(green) + i32::from(blue)) / 3;
        let grey_step = ((average - 8 + 5) / 10).clamp(0, 23) as u8;
        let grey_value = 8 + 10 * i32::from(grey_step);
        let grey_index = 232 + grey_step;

        if distance((grey_value, grey_value, grey_value)) < distance(cube_rgb(cube_index)) {
            grey_index
        } else {
            cube_index
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token {
    rgb: Rgb,
    indexed: u8,
}

const fn token(red: u8, green: u8, blue: u8, indexed: u8) -> Token {
    Token { rgb: Rgb(red, green, blue), indexed }
}

const BG: Token = token(0x00, 0x00, 0x00, 16);
const FG_BRIGHT: Token = token(0xed, 0xed, 0xed, 255);
const FG_FOCUS: Token = token(0xff, 0xff, 0xff, 231);
const FG: Token = token(0xb3, 0xb3, 0xb3, 249);
const FG_MUTED: Token = token(0x8a, 0x8a, 0x8a, 245);
const FG_DIM: Token = token(0x6e, 0x6e, 0x6e, 242);
const LINE: Token = token(0x1c, 0x1c, 0x1c, 234);
const LINE_STRONG: Token = token(0x26, 0x26, 0x26, 235);
const ROW_FOCUS: Token = token(0x16, 0x16, 0x16, 233);
const ROW_FOCUS_INACTIVE: Token = token(0x0b, 0x0b, 0x0b, 232);
const SURFACE: Token = token(0x0a, 0x0a, 0x0a, 232);
const ACCENT: Token = token(0x9f, 0xb3, 0xc8, 110);
const OK: Token = token(0x8f, 0xb3, 0x8a, 108);
const OK_DIM: Token = token(0x4f, 0x6b, 0x4c, 65);
const ERR: Token = token(0xc9, 0x8b, 0x84, 174);
const IDLE: Token = token(0x33, 0x33, 0x33, 236);
const PAUSED: Token = token(0x5a, 0x5a, 0x5a, 240);
const INACTIVE_FILE: Token = token(0x7a, 0x7a, 0x7a, 243);
const INACTIVE_DIR: Token = token(0xa0, 0xa0, 0xa0, 247);

fn color_of(depth: ColorDepth, token: Token) -> Color {
    token.rgb.color(depth, token.indexed)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeSettings {
    pub accent: Option<Rgb>,
    pub terminal_background: bool,
}

impl ThemeSettings {
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
        let pick = |token: Token| color_of(depth, token);
        let accent = match settings.accent {
            Some(rgb) if rgb == ACCENT.rgb => pick(ACCENT),
            Some(rgb) => rgb.color(depth, rgb.nearest_indexed()),
            None => pick(ACCENT),
        };
        Self {
            bg: if settings.terminal_background { Color::Reset } else { pick(BG) },
            fg_bright: pick(FG_BRIGHT),
            fg_focus: pick(FG_FOCUS),
            fg: pick(FG),
            fg_muted: pick(FG_MUTED),
            fg_dim: pick(FG_DIM),
            line: pick(LINE),
            line_strong: pick(LINE_STRONG),
            row_focus: pick(ROW_FOCUS),
            row_focus_inactive: pick(ROW_FOCUS_INACTIVE),
            surface: pick(SURFACE),
            accent,
            ok: pick(OK),
            ok_dim: pick(OK_DIM),
            err: pick(ERR),
            idle: pick(IDLE),
            paused: pick(PAUSED),
            inactive_file: pick(INACTIVE_FILE),
            inactive_dir: pick(INACTIVE_DIR),
        }
    }

    pub fn idle_text(&self) -> Style {
        Style::default().fg(self.idle)
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
