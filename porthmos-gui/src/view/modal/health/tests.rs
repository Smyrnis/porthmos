use porthmos_core::ProcessSignal;
use porthmos_core::theme::ColorScheme;

use super::*;
use crate::theme::ThemeSettings;

#[test]
fn the_modals_build_in_all_three_colour_schemes() {
    for scheme in [ColorScheme::dark(), ColorScheme::light(), ColorScheme::high_contrast()] {
        let palette = Palette::new(&ThemeSettings::default().with_scheme(scheme));
        let _ = renice_body("nginx", 10, "5", &None, &palette);
        let _ = renice_body("nginx", 10, "x", &Some("Enter a whole number from -20 to 19".to_string()), &palette);
    }
}

#[test]
fn the_questions_name_the_process_and_use_different_titles_and_buttons() {
    let term = signal_question(ProcessSignal::Terminate, "nginx", 4242);
    let kill = signal_question(ProcessSignal::Kill, "nginx", 4242);

    assert!(term.contains("nginx") && term.contains("4242"), "{term}");
    assert!(kill.contains("nginx") && kill.contains("4242"), "{kill}");
    assert_ne!(term, kill);
    assert_ne!(signal_title(ProcessSignal::Terminate), signal_title(ProcessSignal::Kill));
    assert_ne!(signal_accept(ProcessSignal::Terminate), signal_accept(ProcessSignal::Kill));
}

#[test]
fn a_hostile_process_name_is_cleaned_and_shortened_in_the_question() {
    let name = format!("a\u{7}b\u{202e}{}", "c".repeat(500));

    let text = signal_question(ProcessSignal::Kill, &name, 7);

    assert!(!text.contains('\u{7}') && !text.contains('\u{202e}'), "{text:?}");
    assert!(text.chars().count() < 300, "{}", text.chars().count());
}
