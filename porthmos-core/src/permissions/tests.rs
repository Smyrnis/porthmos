use super::{ModeRequest, mode_text, parse_mode, parse_mode_request, symbolic};

#[test]
fn three_and_four_octal_digits_are_a_mode() {
    assert_eq!(parse_mode("644"), Ok(0o644));
    assert_eq!(parse_mode("755"), Ok(0o755));
    assert_eq!(parse_mode("000"), Ok(0));
    assert_eq!(parse_mode("0644"), Ok(0o644));
    assert_eq!(parse_mode("4755"), Ok(0o4755));
    assert_eq!(parse_mode("1777"), Ok(0o1777));
}

#[test]
fn surrounding_spaces_are_ignored() {
    assert_eq!(parse_mode("  640 "), Ok(0o640));
}

#[test]
fn an_empty_mode_asks_for_one() {
    assert_eq!(parse_mode(""), Err("Type a mode such as 644 or 755".to_string()));
    assert_eq!(parse_mode("   "), Err("Type a mode such as 644 or 755".to_string()));
}

#[test]
fn anything_but_octal_digits_is_refused() {
    let message = Err("A mode is made of the digits 0 to 7".to_string());
    assert_eq!(parse_mode("889"), message);
    assert_eq!(parse_mode("rwx"), message);
    assert_eq!(parse_mode("64a"), message);
    assert_eq!(parse_mode("-644"), message);
    assert_eq!(parse_mode("0o644"), message);
}

#[test]
fn the_wrong_number_of_digits_is_refused() {
    let message = Err("A mode has three or four digits".to_string());
    assert_eq!(parse_mode("7"), message);
    assert_eq!(parse_mode("64"), message);
    assert_eq!(parse_mode("12345"), message);
}

#[test]
fn a_mode_is_written_with_at_least_three_octal_digits() {
    assert_eq!(mode_text(0o644), "644");
    assert_eq!(mode_text(0o7), "007");
    assert_eq!(mode_text(0), "000");
    assert_eq!(mode_text(0o4755), "4755");
}

#[test]
fn the_file_type_bits_are_not_part_of_the_text() {
    assert_eq!(mode_text(0o100644), "644");
    assert_eq!(mode_text(0o040755), "755");
    assert_eq!(mode_text(0o104755), "4755");
}

#[test]
fn the_nine_bits_read_as_rwx_triplets() {
    assert_eq!(symbolic(0o644), "rw-r--r--");
    assert_eq!(symbolic(0o755), "rwxr-xr-x");
    assert_eq!(symbolic(0), "---------");
    assert_eq!(symbolic(0o777), "rwxrwxrwx");
    assert_eq!(symbolic(0o100640), "rw-r-----");
}

#[test]
fn the_special_bits_replace_the_execute_letter_like_ls_does() {
    assert_eq!(symbolic(0o4755), "rwsr-xr-x");
    assert_eq!(symbolic(0o4644), "rwSr--r--");
    assert_eq!(symbolic(0o2755), "rwxr-sr-x");
    assert_eq!(symbolic(0o2745), "rwxr-Sr-x");
    assert_eq!(symbolic(0o1777), "rwxrwxrwt");
    assert_eq!(symbolic(0o1776), "rwxrwxrwT");
}

#[test]
fn a_typed_mode_round_trips_through_the_text() {
    for text in ["000", "644", "755", "4755", "2775", "1777"] {
        assert_eq!(mode_text(parse_mode(text).unwrap()), text);
    }
}

#[test]
fn a_mode_request_without_a_flag_is_not_recursive() {
    assert_eq!(parse_mode_request("755"), Ok(ModeRequest { mode: 0o755, recursive: false }));
}

#[test]
fn the_recursive_flag_may_come_before_or_after_the_mode() {
    let expected = Ok(ModeRequest { mode: 0o640, recursive: true });
    assert_eq!(parse_mode_request("640 -R"), expected);
    assert_eq!(parse_mode_request("-R 640"), expected);
    assert_eq!(parse_mode_request("  -r   640  "), expected);
}

#[test]
fn a_flag_alone_or_repeated_or_unknown_is_refused() {
    assert_eq!(parse_mode_request("-R"), Err("Type a mode such as 644 or 755".to_string()));
    assert_eq!(parse_mode_request("-R -R 644"), Err("Use -R only once".to_string()));
    assert_eq!(parse_mode_request("-x 644"), Err("The only option is -R (everything inside folders)".to_string()));
}

#[test]
fn two_modes_are_refused() {
    assert_eq!(
        parse_mode_request("644 755"),
        Err("Type one mode, and -R to include what is inside folders".to_string())
    );
}

#[test]
fn a_bad_mode_next_to_the_flag_keeps_its_own_message() {
    assert_eq!(parse_mode_request("-R 899"), Err("A mode is made of the digits 0 to 7".to_string()));
}
