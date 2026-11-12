use super::rule_color;
use crate::theme::{Palette, ThemeSettings};

fn palette() -> Palette {
    Palette::new(&ThemeSettings::default())
}

#[test]
fn the_rule_under_the_active_panels_header_is_the_accent() {
    let palette = palette();

    assert_eq!(rule_color(true, &palette), palette.accent);
}

#[test]
fn the_rule_under_the_inactive_panels_header_is_the_plain_line() {
    let palette = palette();

    assert_eq!(rule_color(false, &palette), palette.line);
}
