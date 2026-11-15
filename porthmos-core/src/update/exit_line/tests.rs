use super::*;

#[test]
fn an_installed_update_is_announced_on_stdout() {
    let outcome = ApplyOutcome::Installed { version: "2026.5.0".into() };

    assert_eq!(outcome.exit_line("porthmos"), Some(ExitLine::Out("Porthmos updated to 2026.5.0.".to_string())));
    assert_eq!(outcome.exit_line("porthmos-gui"), Some(ExitLine::Out("Porthmos updated to 2026.5.0.".to_string())));
}

#[test]
fn a_failed_update_is_reported_on_stderr_with_the_program_name() {
    let outcome = ApplyOutcome::Failed { version: "2026.5.0".into(), message: "disk full".into() };

    assert_eq!(
        outcome.exit_line("porthmos"),
        Some(ExitLine::Err("porthmos: couldn't install the update to 2026.5.0: disk full".to_string()))
    );
    assert_eq!(
        outcome.exit_line("porthmos-gui"),
        Some(ExitLine::Err("porthmos-gui: couldn't install the update to 2026.5.0: disk full".to_string()))
    );
}

#[test]
fn nothing_to_install_says_nothing() {
    assert_eq!(ApplyOutcome::Nothing.exit_line("porthmos"), None);
}
