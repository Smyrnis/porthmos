use super::*;

#[test]
fn rgb_parse_accepts_only_seven_character_hex() {
    assert_eq!(Rgb::parse("#9fb3c8"), Some(Rgb(0x9f, 0xb3, 0xc8)));
    assert_eq!(Rgb::parse("#9FB3C8"), Some(Rgb(0x9f, 0xb3, 0xc8)));
    for bad in ["9fb3c8", "#9fb3c", "#9fb3c8f", "#gggggg", "", "#", "red", "#9fb3c8 "] {
        assert_eq!(Rgb::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn nearest_indexed_maps_into_the_colour_cube_and_the_grey_ramp() {
    assert_eq!(Rgb(0, 0, 0).nearest_indexed(), 16);
    assert_eq!(Rgb(255, 255, 255).nearest_indexed(), 231);
    assert_eq!(Rgb(255, 0, 0).nearest_indexed(), 196);
    assert_eq!(Rgb(0x1c, 0x1c, 0x1c).nearest_indexed(), 234);
    assert_eq!(Rgb(0x6e, 0x6e, 0x6e).nearest_indexed(), 242);
}

#[test]
fn nearest_indexed_matches_the_xterm_palette_at_saturated_and_boundary_inputs() {
    let expected = [
        (Rgb(255, 0, 255), 201),
        (Rgb(0, 255, 0), 46),
        (Rgb(0, 0, 255), 21),
        (Rgb(255, 255, 0), 226),
        (Rgb(0, 255, 255), 51),
        (Rgb(95, 95, 95), 59),
        (Rgb(135, 135, 135), 102),
        (Rgb(3, 3, 3), 16),
        (Rgb(4, 4, 4), 16),
        (Rgb(5, 5, 5), 232),
        (Rgb(8, 8, 8), 232),
        (Rgb(18, 18, 18), 233),
        (Rgb(48, 48, 48), 236),
        (Rgb(128, 128, 128), 244),
        (Rgb(238, 238, 238), 255),
        (Rgb(47, 255, 0), 46),
        (Rgb(48, 255, 0), 82),
        (Rgb(194, 0, 0), 124),
        (Rgb(196, 0, 0), 160),
    ];

    for (rgb, index) in expected {
        assert_eq!(rgb.nearest_indexed(), index, "{rgb:?}");
    }
}

#[test]
fn nearest_indexed_never_returns_a_system_colour_or_panics() {
    let channel_values = (0..=255u8).step_by(15).chain([1, 94, 95, 96, 254, 255]);
    let channel_values: Vec<u8> = channel_values.collect();

    for red in &channel_values {
        for green in &channel_values {
            for blue in &channel_values {
                let index = Rgb(*red, *green, *blue).nearest_indexed();
                assert!(index >= 16, "{red} {green} {blue} -> {index}");
            }
        }
    }
}

#[test]
fn the_dark_scheme_has_these_values() {
    let scheme = ColorScheme::dark();

    assert_eq!(scheme.bg, Rgb(0x00, 0x00, 0x00));
    assert_eq!(scheme.fg_bright, Rgb(0xf2, 0xf2, 0xf2));
    assert_eq!(scheme.fg_focus, Rgb(0xff, 0xff, 0xff));
    assert_eq!(scheme.fg, Rgb(0xd0, 0xd0, 0xd0));
    assert_eq!(scheme.fg_muted, Rgb(0xb0, 0xb0, 0xb0));
    assert_eq!(scheme.fg_dim, Rgb(0x9a, 0x9a, 0x9a));
    assert_eq!(scheme.line, Rgb(0x2e, 0x2e, 0x2e));
    assert_eq!(scheme.line_strong, Rgb(0x55, 0x55, 0x55));
    assert_eq!(scheme.row_focus, Rgb(0x22, 0x2b, 0x36));
    assert_eq!(scheme.row_focus_inactive, Rgb(0x16, 0x1a, 0x20));
    assert_eq!(scheme.surface, Rgb(0x10, 0x12, 0x15));
    assert_eq!(scheme.accent, Rgb(0x9f, 0xb3, 0xc8));
    assert_eq!(scheme.ok, Rgb(0x8f, 0xb3, 0x8a));
    assert_eq!(scheme.ok_dim, Rgb(0x84, 0xa3, 0x80));
    assert_eq!(scheme.err, Rgb(0xd9, 0x9a, 0x93));
    assert_eq!(scheme.idle, Rgb(0x4a, 0x4a, 0x4a));
    assert_eq!(scheme.paused, Rgb(0x96, 0x96, 0x96));
    assert_eq!(scheme.inactive_file, Rgb(0xa4, 0xa4, 0xa4));
    assert_eq!(scheme.inactive_dir, Rgb(0xbc, 0xbc, 0xbc));
}

#[test]
fn a_custom_accent_replaces_only_the_accent() {
    let custom = Rgb(0x10, 0x20, 0x30);

    let scheme = ColorScheme::dark().with_accent(Some(custom));

    assert_eq!(scheme.accent, custom);
    assert_eq!(ColorScheme { accent: ColorScheme::dark().accent, ..scheme }, ColorScheme::dark());
}

#[test]
fn no_accent_keeps_the_default() {
    assert_eq!(ColorScheme::dark().with_accent(None), ColorScheme::dark());
}

#[test]
fn contrast_is_21_between_black_and_white_and_1_between_equals() {
    assert!((Rgb(0, 0, 0).contrast(Rgb(255, 255, 255)) - 21.0).abs() < 1e-9);
    assert!((Rgb(255, 255, 255).contrast(Rgb(0, 0, 0)) - 21.0).abs() < 1e-9);
    assert!((Rgb(0x70, 0x80, 0x90).contrast(Rgb(0x70, 0x80, 0x90)) - 1.0).abs() < 1e-9);
}

#[test]
fn contrast_matches_the_published_wcag_examples() {
    assert!((Rgb(0x77, 0x77, 0x77).contrast(Rgb(255, 255, 255)) - 4.48).abs() < 0.01);
    assert!((Rgb(0x76, 0x76, 0x76).contrast(Rgb(255, 255, 255)) - 4.54).abs() < 0.01);
}

const TEXT: f64 = 4.5;
const BORDER: f64 = 2.0;
const SEPARATOR: f64 = 1.5;
const GRAPHIC: f64 = 2.0;
const SELECTION: f64 = 1.4;
const INACTIVE_SELECTION: f64 = 1.2;

fn text_roles(scheme: &ColorScheme) -> [(&'static str, Rgb); 12] {
    [
        ("fg_bright", scheme.fg_bright),
        ("fg_focus", scheme.fg_focus),
        ("fg", scheme.fg),
        ("fg_muted", scheme.fg_muted),
        ("fg_dim", scheme.fg_dim),
        ("accent", scheme.accent),
        ("ok", scheme.ok),
        ("ok_dim", scheme.ok_dim),
        ("err", scheme.err),
        ("paused", scheme.paused),
        ("inactive_file", scheme.inactive_file),
        ("inactive_dir", scheme.inactive_dir),
    ]
}

fn assert_readable(scheme: &ColorScheme) {
    let grounds = [
        ("bg", scheme.bg),
        ("surface", scheme.surface),
        ("row_focus", scheme.row_focus),
        ("row_focus_inactive", scheme.row_focus_inactive),
    ];
    for (role, colour) in text_roles(scheme) {
        for (ground_name, ground) in grounds {
            let ratio = colour.contrast(ground);
            assert!(ratio >= TEXT, "{role} on {ground_name} is {ratio:.2}:1, needs {TEXT}:1");
        }
    }
}

fn assert_visible(scheme: &ColorScheme) {
    let checks = [
        ("line", scheme.line, SEPARATOR),
        ("line_strong", scheme.line_strong, BORDER),
        ("idle", scheme.idle, GRAPHIC),
        ("row_focus", scheme.row_focus, SELECTION),
        ("row_focus_inactive", scheme.row_focus_inactive, INACTIVE_SELECTION),
    ];
    for (role, colour, needed) in checks {
        let ratio = colour.contrast(scheme.bg);
        assert!(ratio >= needed, "{role} against bg is {ratio:.2}:1, needs {needed}:1");
    }
}

#[test]
fn every_text_colour_of_the_dark_scheme_is_readable_on_every_ground() {
    assert_readable(&ColorScheme::dark());
}

#[test]
fn the_lines_graphics_and_selection_of_the_dark_scheme_can_be_seen() {
    assert_visible(&ColorScheme::dark());
}

#[test]
fn the_dark_text_ladder_gets_dimmer_step_by_step() {
    let scheme = ColorScheme::dark();
    let ladder = [scheme.fg_focus, scheme.fg_bright, scheme.fg, scheme.fg_muted, scheme.fg_dim];

    for pair in ladder.windows(2) {
        assert!(pair[0].contrast(scheme.bg) > pair[1].contrast(scheme.bg), "{pair:?}");
    }
}

#[test]
fn the_selected_row_is_more_visible_in_the_active_panel_than_in_the_other() {
    let scheme = ColorScheme::dark();

    assert!(scheme.row_focus.contrast(scheme.bg) > scheme.row_focus_inactive.contrast(scheme.bg));
}
