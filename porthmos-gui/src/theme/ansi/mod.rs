use iced::Color;
use porthmos_term::Color as TermColor;

use super::Palette;

const ANSI_COLOURS: usize = 16;
const CUBE_START: u8 = 16;
const GRAYSCALE_START: u8 = 232;
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
const GRAYSCALE_BASE: u8 = 8;
const GRAYSCALE_STEP: u8 = 10;

const DARK: [[u8; 3]; ANSI_COLOURS] = [
    [0x00, 0x00, 0x00],
    [0xcc, 0x66, 0x66],
    [0xb5, 0xbd, 0x68],
    [0xf0, 0xc6, 0x74],
    [0x81, 0xa2, 0xbe],
    [0xb2, 0x94, 0xbb],
    [0x8a, 0xbe, 0xb7],
    [0xc5, 0xc8, 0xc6],
    [0x96, 0x98, 0x96],
    [0xd5, 0x4e, 0x53],
    [0xb9, 0xca, 0x4a],
    [0xe7, 0xc5, 0x47],
    [0x7a, 0xa6, 0xda],
    [0xc3, 0x97, 0xd8],
    [0x70, 0xc0, 0xb1],
    [0xea, 0xea, 0xea],
];

const LIGHT: [[u8; 3]; ANSI_COLOURS] = [
    [0x00, 0x00, 0x00],
    [0xa4, 0x0e, 0x26],
    [0x11, 0x63, 0x29],
    [0x6a, 0x4a, 0x00],
    [0x05, 0x50, 0xae],
    [0x66, 0x39, 0xba],
    [0x1b, 0x6e, 0x75],
    [0x57, 0x60, 0x6a],
    [0x3b, 0x42, 0x4a],
    [0x8f, 0x0a, 0x20],
    [0x0e, 0x56, 0x23],
    [0x59, 0x3e, 0x00],
    [0x04, 0x45, 0x9c],
    [0x57, 0x2f, 0x9e],
    [0x15, 0x5d, 0x63],
    [0x6e, 0x77, 0x81],
];

fn rgb([red, green, blue]: [u8; 3]) -> Color {
    Color::from_rgb8(red, green, blue)
}

pub fn resolve(color: TermColor, is_foreground: bool, palette: &Palette) -> Color {
    match color {
        TermColor::Default if is_foreground => palette.fg,
        TermColor::Default => palette.bg,
        TermColor::Rgb(red, green, blue) => Color::from_rgb8(red, green, blue),
        TermColor::Indexed(index) if usize::from(index) < ANSI_COLOURS => {
            let table = if palette.light { &LIGHT } else { &DARK };
            rgb(table[usize::from(index)])
        }
        TermColor::Indexed(index) if index >= GRAYSCALE_START => {
            let level = GRAYSCALE_BASE + GRAYSCALE_STEP * (index - GRAYSCALE_START);
            Color::from_rgb8(level, level, level)
        }
        TermColor::Indexed(index) => {
            let offset = usize::from(index - CUBE_START);
            let level = |position: usize| CUBE_LEVELS[position % CUBE_LEVELS.len()];
            Color::from_rgb8(level(offset / 36), level(offset / 6), level(offset))
        }
    }
}

#[cfg(test)]
mod tests;
