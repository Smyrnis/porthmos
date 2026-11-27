use porthmos_core::Privilege;
use porthmos_core::theme::ColorScheme;

use super::*;
use crate::theme::ThemeSettings;

const VERBS: [ServiceVerb; 5] =
    [ServiceVerb::Start, ServiceVerb::Stop, ServiceVerb::Restart, ServiceVerb::Enable, ServiceVerb::Disable];

#[test]
fn the_body_builds_in_all_three_colour_schemes_for_both_privileges() {
    for scheme in [ColorScheme::dark(), ColorScheme::light(), ColorScheme::high_contrast()] {
        let palette = Palette::new(&ThemeSettings::default().with_scheme(scheme));
        for privilege in [Privilege::Direct, Privilege::Sudo] {
            let _ = body("nginx.service", ServiceVerb::Restart, privilege, &palette);
        }
    }
}

#[test]
fn each_verb_has_its_own_title_accept_button_and_question() {
    let titles: Vec<String> = VERBS.iter().map(|verb| title(*verb)).collect();
    let accepts: Vec<String> = VERBS.iter().map(|verb| accept(*verb)).collect();
    let questions: Vec<String> = VERBS.iter().map(|verb| question(*verb, "nginx.service")).collect();

    for list in [&titles, &accepts, &questions] {
        let mut unique = list.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), VERBS.len(), "{list:?}");
    }
    assert!(questions.iter().all(|question| question.contains("nginx.service")), "{questions:?}");
}

#[test]
fn the_sudo_note_appears_exactly_when_the_command_runs_with_sudo() {
    assert_eq!(sudo_note(Privilege::Direct), None);
    let note = sudo_note(Privilege::Sudo).expect("a note");
    assert!(note.contains("sudo"), "{note}");
    assert_eq!(Privilege::for_uid(0), Privilege::Direct);
    assert_eq!(Privilege::for_uid(1000), Privilege::Sudo);
    assert_eq!(Privilege::for_uid(u32::MAX), Privilege::Sudo);
}

#[test]
fn a_hostile_unit_name_is_cleaned_and_shortened_in_the_question() {
    let name = format!("a\u{7}b\u{202e}{}", "c".repeat(500));

    let text = question(ServiceVerb::Stop, &name);

    assert!(!text.contains('\u{7}') && !text.contains('\u{202e}'), "{text:?}");
    assert!(text.chars().count() < 300, "{}", text.chars().count());
}
