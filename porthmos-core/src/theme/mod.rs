pub mod file;

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

    fn luminance(self) -> f64 {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.03928 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }

    pub fn contrast(self, other: Rgb) -> f64 {
        let (first, second) = (self.luminance(), other.luminance());
        let (lighter, darker) = if first >= second { (first, second) } else { (second, first) };
        (lighter + 0.05) / (darker + 0.05)
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
pub struct ColorScheme {
    pub bg: Rgb,
    pub fg_bright: Rgb,
    pub fg_focus: Rgb,
    pub fg: Rgb,
    pub fg_muted: Rgb,
    pub fg_dim: Rgb,
    pub line: Rgb,
    pub line_strong: Rgb,
    pub row_focus: Rgb,
    pub row_focus_inactive: Rgb,
    pub surface: Rgb,
    pub accent: Rgb,
    pub ok: Rgb,
    pub ok_dim: Rgb,
    pub err: Rgb,
    pub idle: Rgb,
    pub paused: Rgb,
    pub inactive_file: Rgb,
    pub inactive_dir: Rgb,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl ColorScheme {
    pub const fn dark() -> Self {
        Self {
            bg: Rgb(0x00, 0x00, 0x00),
            fg_bright: Rgb(0xf2, 0xf2, 0xf2),
            fg_focus: Rgb(0xff, 0xff, 0xff),
            fg: Rgb(0xd0, 0xd0, 0xd0),
            fg_muted: Rgb(0xb0, 0xb0, 0xb0),
            fg_dim: Rgb(0x9a, 0x9a, 0x9a),
            line: Rgb(0x2e, 0x2e, 0x2e),
            line_strong: Rgb(0x55, 0x55, 0x55),
            row_focus: Rgb(0x22, 0x2b, 0x36),
            row_focus_inactive: Rgb(0x16, 0x1a, 0x20),
            surface: Rgb(0x10, 0x12, 0x15),
            accent: Rgb(0x9f, 0xb3, 0xc8),
            ok: Rgb(0x8f, 0xb3, 0x8a),
            ok_dim: Rgb(0x84, 0xa3, 0x80),
            err: Rgb(0xd9, 0x9a, 0x93),
            idle: Rgb(0x4a, 0x4a, 0x4a),
            paused: Rgb(0x96, 0x96, 0x96),
            inactive_file: Rgb(0xa4, 0xa4, 0xa4),
            inactive_dir: Rgb(0xbc, 0xbc, 0xbc),
        }
    }

    pub const ROLES: [&'static str; 19] = [
        "bg",
        "fg_bright",
        "fg_focus",
        "fg",
        "fg_muted",
        "fg_dim",
        "line",
        "line_strong",
        "row_focus",
        "row_focus_inactive",
        "surface",
        "accent",
        "ok",
        "ok_dim",
        "err",
        "idle",
        "paused",
        "inactive_file",
        "inactive_dir",
    ];

    pub fn role_mut(&mut self, name: &str) -> Option<&mut Rgb> {
        Some(match name {
            "bg" => &mut self.bg,
            "fg_bright" => &mut self.fg_bright,
            "fg_focus" => &mut self.fg_focus,
            "fg" => &mut self.fg,
            "fg_muted" => &mut self.fg_muted,
            "fg_dim" => &mut self.fg_dim,
            "line" => &mut self.line,
            "line_strong" => &mut self.line_strong,
            "row_focus" => &mut self.row_focus,
            "row_focus_inactive" => &mut self.row_focus_inactive,
            "surface" => &mut self.surface,
            "accent" => &mut self.accent,
            "ok" => &mut self.ok,
            "ok_dim" => &mut self.ok_dim,
            "err" => &mut self.err,
            "idle" => &mut self.idle,
            "paused" => &mut self.paused,
            "inactive_file" => &mut self.inactive_file,
            "inactive_dir" => &mut self.inactive_dir,
            _ => return None,
        })
    }

    pub fn with_accent(self, accent: Option<Rgb>) -> Self {
        Self { accent: accent.unwrap_or(self.accent), ..self }
    }
}

#[cfg(test)]
mod tests;
