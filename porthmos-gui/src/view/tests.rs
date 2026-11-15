use iced::Color;

use super::{SCRIM_ALPHA, scrim_color};
use crate::theme::{Palette, ThemeSettings};

#[test]
fn the_scrim_is_the_window_background_made_translucent() {
    let palette = Palette::new(&ThemeSettings::default());

    let scrim = scrim_color(&palette);

    assert_eq!((scrim.r, scrim.g, scrim.b), (palette.bg.r, palette.bg.g, palette.bg.b));
    assert_eq!(scrim.a, SCRIM_ALPHA);
}

#[test]
fn the_scrim_hides_enough_to_dim_but_not_so_much_that_the_page_vanishes() {
    assert!((0.4..=0.8).contains(&SCRIM_ALPHA));
    assert_ne!(scrim_color(&Palette::new(&ThemeSettings::default())), Color::TRANSPARENT);
}
