use std::ffi::OsString;

use super::*;

fn arguments(prompt: &str) -> Vec<OsString> {
    vec![OsString::from(prompt)]
}

fn lookup_with(user: Option<&'static str>, password: Option<&'static str>) -> impl Fn(&str) -> Option<String> {
    move |name| match name {
        USER_VARIABLE => user.map(str::to_string),
        PASSWORD_VARIABLE => password.map(str::to_string),
        _ => None,
    }
}

fn run(prompt: &str, lookup: impl Fn(&str) -> Option<String>) -> (i32, Vec<u8>) {
    let mut out = Vec::new();
    let code = run_helper(&arguments(prompt), lookup, &mut out);
    (code, out)
}

#[test]
fn the_exact_password_prompt_for_the_user_is_answered() {
    assert_eq!(answer_for("deploy@server's password: ", "deploy", "pw"), Some("pw".to_string()));
    assert_eq!(answer_for("deploy@10.0.0.2's password: ", "deploy", "pw"), Some("pw".to_string()));
    assert_eq!(answer_for("deploy@[::1]'s password: ", "deploy", "pw"), Some("pw".to_string()));
}

#[test]
fn a_key_passphrase_prompt_is_refused_even_when_the_path_says_password() {
    assert_eq!(answer_for("Enter passphrase for key '/home/u/passwords/id': ", "deploy", "pw"), None);
    assert_eq!(answer_for("Enter passphrase for key '/home/deploy@x/'s password: ': ", "deploy", "pw"), None);
}

#[test]
fn a_prompt_for_another_user_is_refused() {
    assert_eq!(answer_for("root@server's password: ", "deploy", "pw"), None);
    assert_eq!(answer_for("deploy2@server's password: ", "deploy", "pw"), None);
    assert_eq!(answer_for("xdeploy@server's password: ", "deploy", "pw"), None);
}

#[test]
fn a_user_with_regex_like_characters_matches_only_literally() {
    assert_eq!(answer_for("a.b@server's password: ", "a.b", "pw"), Some("pw".to_string()));
    assert_eq!(answer_for("axb@server's password: ", "a.b", "pw"), None);
}

#[test]
fn a_cut_or_padded_prompt_is_refused() {
    assert_eq!(answer_for("deploy@server's password:", "deploy", "pw"), None);
    assert_eq!(answer_for("deploy@server's password: extra", "deploy", "pw"), None);
    assert_eq!(answer_for(" deploy@server's password: ", "deploy", "pw"), None);
    assert_eq!(answer_for("deploy@server's password: \n", "deploy", "pw"), None);
    assert_eq!(answer_for("deploy@server's password: \nmore", "deploy", "pw"), None);
}

#[test]
fn a_prompt_without_a_usable_host_is_refused() {
    assert_eq!(answer_for("deploy@'s password: ", "deploy", "pw"), None);
    assert_eq!(answer_for("deploy@a b's password: ", "deploy", "pw"), None);
    assert_eq!(answer_for("deploy@a\tb's password: ", "deploy", "pw"), None);
    assert_eq!(answer_for("deploy@a\u{7}b's password: ", "deploy", "pw"), None);
}

#[test]
fn other_questions_are_refused() {
    let host_key = "The authenticity of host 'h (1.2.3.4)' can't be established.\nED25519 key fingerprint is SHA256:x.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
    for prompt in [
        host_key,
        "(deploy@server) Verification code: ",
        "(deploy@server) Password:",
        "(deploy@server) Password: x",
        "(root@server) Password: ",
        "(deploy@) Password: ",
        "(deploy@a b) Password: ",
        "(deploy@server)Password: ",
        "(deploy@server) password: ",
        "",
        "Password: ",
    ] {
        assert_eq!(answer_for(prompt, "deploy", "pw"), None, "{prompt:?}");
    }
}

#[test]
fn the_keyboard_interactive_password_prompt_with_user_and_host_is_answered() {
    assert_eq!(answer_for("(deploy@server) Password: ", "deploy", "pw"), Some("pw".to_string()));
    assert_eq!(answer_for("(deploy@10.0.0.2) Password: ", "deploy", "pw"), Some("pw".to_string()));
}

#[test]
fn a_host_with_an_at_sign_is_refused() {
    assert_eq!(answer_for("deploy@root@h's password: ", "deploy", "pw"), None);
    assert_eq!(answer_for("(deploy@root@h) Password: ", "deploy", "pw"), None);
}

#[test]
fn an_empty_user_never_matches() {
    assert_eq!(answer_for("@server's password: ", "", "pw"), None);
}

#[test]
fn a_password_over_the_limit_or_with_a_newline_or_nul_is_not_usable() {
    assert!(usable_password(&"a".repeat(1023)));
    assert!(!usable_password(&"a".repeat(1024)));
    assert!(!usable_password("two\nlines"));
    assert!(!usable_password("nul\0inside"));
    assert!(!usable_password(""));
}

#[test]
fn quotes_spaces_dollars_and_unicode_are_usable() {
    for password in ["it's \"quoted\"", "with space", "$HOME`x`", "p\u{e1}ss\u{3b1}"] {
        assert!(usable_password(password), "{password}");
        assert_eq!(answer_for("u@h's password: ", "u", password), Some(password.to_string()));
    }
}

#[test]
fn the_multibyte_limit_counts_bytes() {
    assert!(usable_password(&"\u{3b1}".repeat(511)));
    assert!(!usable_password(&"\u{3b1}".repeat(512)));
}

#[test]
fn the_helper_writes_only_the_password_and_a_newline() {
    let (code, out) = run("deploy@server's password: ", lookup_with(Some("deploy"), Some("s3cret")));

    assert_eq!(code, 0);
    assert_eq!(out, b"s3cret\n");
}

#[test]
fn a_refused_prompt_writes_nothing_and_fails() {
    let (code, out) = run("Enter passphrase for key '/k': ", lookup_with(Some("deploy"), Some("s3cret")));

    assert_eq!(code, 1);
    assert!(out.is_empty());
}

#[test]
fn a_missing_variable_writes_nothing_and_fails() {
    for lookup in [lookup_with(None, Some("s3cret")), lookup_with(Some("deploy"), None), lookup_with(None, None)] {
        let (code, out) = run("deploy@server's password: ", lookup);
        assert_eq!(code, 1);
        assert!(out.is_empty());
    }
}

#[test]
fn no_prompt_argument_writes_nothing_and_fails() {
    let mut out = Vec::new();

    let code = run_helper(&[], lookup_with(Some("deploy"), Some("s3cret")), &mut out);

    assert_eq!(code, 1);
    assert!(out.is_empty());
}

#[test]
fn a_prompt_that_is_not_valid_text_is_refused() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;

        let mut out = Vec::new();
        let prompt = OsString::from_vec(b"deploy@\xff's password: ".to_vec());

        let code = run_helper(&[prompt], lookup_with(Some("deploy"), Some("s3cret")), &mut out);

        assert_eq!(code, 1);
        assert!(out.is_empty());
    }
}

#[test]
fn the_variable_names_are_fixed() {
    assert_eq!(HELPER_SWITCH, "PORTHMOS_ASKPASS");
    assert_eq!(PASSWORD_VARIABLE, "PORTHMOS_ASKPASS_PASSWORD");
    assert_eq!(USER_VARIABLE, "PORTHMOS_ASKPASS_USER");
    assert_eq!(MAX_PASSWORD_BYTES, 1023);
}
