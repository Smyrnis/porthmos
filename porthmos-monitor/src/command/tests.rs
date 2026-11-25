use super::*;

#[test]
fn signal_and_renice_commands_are_exact_and_numeric_only() {
    assert_eq!(signal_command(1234, Signal::Terminate).unwrap(), "kill -TERM 1234");
    assert_eq!(signal_command(1234, Signal::Kill).unwrap(), "kill -KILL 1234");
    assert_eq!(renice_command(1234, Nice::new(10).unwrap()).unwrap(), "renice 10 -p 1234");
    assert_eq!(renice_command(77, Nice::new(-5).unwrap()).unwrap(), "renice -5 -p 77");
}

#[test]
fn pid_zero_and_one_are_refused() {
    for pid in [0, 1] {
        assert_eq!(signal_command(pid, Signal::Kill), Err(CommandError::PidNotAllowed));
        assert_eq!(renice_command(pid, Nice::new(0).unwrap()), Err(CommandError::PidNotAllowed));
    }
    assert!(signal_command(2, Signal::Terminate).is_ok());
}

#[test]
fn nice_outside_minus_20_to_19_is_refused() {
    assert!(Nice::new(-21).is_none());
    assert!(Nice::new(20).is_none());
    assert!(Nice::new(i32::MIN).is_none());
    assert!(Nice::new(-20).is_some() && Nice::new(19).is_some());
    assert_eq!(Nice::new(-20).unwrap().get(), -20);
}

#[test]
fn every_pid_and_nice_produces_only_fixed_words_digits_and_a_sign() {
    for pid in (2..70_000u32).step_by(997).chain([u32::MAX]) {
        for nice in -20..=19 {
            let command = renice_command(pid, Nice::new(nice).unwrap()).unwrap();
            assert!(
                command.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ' || "renic-p".contains(c)),
                "{command}"
            );
        }
        let command = signal_command(pid, Signal::Terminate).unwrap();
        assert!(command.chars().all(|c| c.is_ascii_digit() || c == ' ' || "kill-TERM".contains(c)), "{command}");
    }
}

#[test]
fn uid_zero_may_signal_anyone_and_others_only_their_own() {
    assert!(may_signal(0, 33));
    assert!(may_signal(0, u32::MAX));
    assert!(may_signal(1000, 1000));
    assert!(!may_signal(1000, 0));
    assert!(!may_signal(1000, 33));
    assert!(!may_signal(1000, u32::MAX));
    assert!(!may_signal(u32::MAX, 0));
}

#[test]
fn a_change_runs_in_a_fixed_shell_with_the_c_locale_so_its_errors_can_be_recognised() {
    assert_eq!(SHELL_COMMAND, "sh -s");
    assert_eq!(change_script("kill -TERM 5"), "LC_ALL=C; export LC_ALL\nkill -TERM 5\n");
}
