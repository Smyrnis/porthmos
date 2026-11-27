use super::*;
use crate::UnitName;

fn script(path: &str) -> Result<String, LogSourceError> {
    log_script(&LogSource::File(path.to_string()), 100)
}

fn unit(name: &str) -> UnitName {
    UnitName::new(name).unwrap()
}

fn split_all(chunks: &[&[u8]]) -> Vec<LogLine> {
    let mut splitter = LineSplitter::new();
    let mut out = Vec::new();
    for chunk in chunks {
        splitter.feed(chunk, &mut out);
    }
    splitter.finish(&mut out);
    out
}

fn texts(chunks: &[&[u8]]) -> Vec<String> {
    split_all(chunks).into_iter().map(|line| line.text).collect()
}

#[test]
fn file_script_has_exact_text() {
    assert_eq!(
        script("/var/log/syslog").unwrap(),
        "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- '/var/log/syslog' 2>&1 </dev/null\n"
    );
}

#[test]
fn hostile_paths_are_quoted_exactly() {
    assert_eq!(script("it's").unwrap(), "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- 'it'\\''s' 2>&1 </dev/null\n");
    assert_eq!(script("$(x)").unwrap(), "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- '$(x)' 2>&1 </dev/null\n");
    assert_eq!(script("-rf").unwrap(), "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- '-rf' 2>&1 </dev/null\n");
    assert_eq!(
        script("a b;`c`\"d\\").unwrap(),
        "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- 'a b;`c`\"d\\' 2>&1 </dev/null\n"
    );
}

#[test]
fn bad_paths_are_refused() {
    assert_eq!(script(""), Err(LogSourceError::EmptyPath));
    assert_eq!(script("a\0b"), Err(LogSourceError::BadCharacter));
    assert_eq!(script("a\nb"), Err(LogSourceError::BadCharacter));
    assert_eq!(MAX_LOG_PATH_BYTES, 4096);
    assert_eq!(script(&"a".repeat(MAX_LOG_PATH_BYTES + 1)), Err(LogSourceError::PathTooLong));
    assert!(script(&"a".repeat(MAX_LOG_PATH_BYTES)).is_ok());
}

#[test]
fn initial_lines_are_clamped() {
    let script = log_script(&LogSource::File("/x".to_string()), u32::MAX).unwrap();
    assert!(script.contains("tail -n 5000 -F"), "{script}");
    let script = log_script(&LogSource::Journal(None), 5001).unwrap();
    assert!(script.contains("journalctl -f -n 5000 "), "{script}");
    let script = log_script(&LogSource::Journal(None), 7).unwrap();
    assert!(script.contains("-n 7 "), "{script}");
}

#[test]
fn journal_script_has_exact_text() {
    assert_eq!(
        log_script(&LogSource::Journal(None), 50).unwrap(),
        "LC_ALL=C; export LC_ALL\nexec journalctl -f -n 50 --no-pager --all -o short-iso 2>&1 </dev/null\n"
    );
    assert_eq!(
        log_script(&LogSource::Journal(Some(unit("ssh.service"))), 50).unwrap(),
        "LC_ALL=C; export LC_ALL\nexec journalctl -f -n 50 --no-pager --all -o short-iso -u 'ssh.service' 2>&1 </dev/null\n"
    );
}

#[test]
fn a_leading_home_path_expands_home_and_quotes_the_rest() {
    assert_eq!(
        script("~/logs/it's.log").unwrap(),
        "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- \"$HOME\"/'logs/it'\\''s.log' 2>&1 </dev/null\n"
    );
    assert_eq!(script("~/").unwrap(), "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- \"$HOME\"/'' 2>&1 </dev/null\n");
}

#[test]
fn other_tildes_stay_literal() {
    assert_eq!(script("~").unwrap(), "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- '~' 2>&1 </dev/null\n");
    assert_eq!(
        script("~root/x").unwrap(),
        "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- '~root/x' 2>&1 </dev/null\n"
    );
    assert_eq!(script("/a/~/b").unwrap(), "LC_ALL=C; export LC_ALL\nexec tail -n 100 -F -- '/a/~/b' 2>&1 </dev/null\n");
}

#[cfg(unix)]
fn run_in_sh_without_path(script: &str) -> Option<std::process::Output> {
    use std::io::Write;
    let mut child = std::process::Command::new("/bin/sh")
        .arg("-s")
        .env("PATH", "/nonexistent-porthmos-path")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(script.as_bytes()).ok()?;
    child.wait_with_output().ok()
}

#[cfg(unix)]
#[test]
fn a_missing_command_still_exits_127_with_stderr_redirected() {
    for source in [LogSource::File("/var/log/syslog".to_string()), LogSource::Journal(None)] {
        let script = log_script(&source, 10).unwrap();
        assert!(script.contains(" 2>&1 </dev/null"), "{script}");
        let Some(output) = run_in_sh_without_path(&script) else { return };
        assert_eq!(output.status.code(), Some(127), "{source:?} {output:?}");
    }
}

#[test]
fn invalid_unit_names_cannot_be_built() {
    assert!(UnitName::new("a'; reboot; '.service").is_none());
    assert!(UnitName::new("-x.service").is_none());
}

#[test]
fn splits_on_lf_and_crlf() {
    assert_eq!(texts(&[b"one\ntwo\r\nthree\n"]), ["one", "two", "three"]);
}

#[test]
fn keeps_blank_lines() {
    assert_eq!(texts(&[b"a\n\nb\n"]), ["a", "", "b"]);
}

#[test]
fn joins_a_line_split_across_feeds() {
    assert_eq!(texts(&[b"hel", b"lo\nwor", b"ld\n"]), ["hello", "world"]);
}

#[test]
fn finish_flushes_unterminated_last_line() {
    assert_eq!(texts(&[b"a\npartial"]), ["a", "partial"]);
    assert_eq!(texts(&[b"a\n"]), ["a"]);
    assert!(texts(&[b""]).is_empty());
}

#[test]
fn finish_twice_does_not_repeat_a_line() {
    let mut splitter = LineSplitter::new();
    let mut out = Vec::new();
    splitter.feed(b"x", &mut out);
    splitter.finish(&mut out);
    splitter.finish(&mut out);
    assert_eq!(out.len(), 1);
}

#[test]
fn over_long_line_is_truncated_and_the_rest_discarded() {
    let mut input = vec![b'a'; MAX_LOG_LINE_BYTES * 3];
    input.extend_from_slice(b"\nnext\n");
    let lines = split_all(&[&input]);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].text.ends_with('…'));
    assert_eq!(lines[0].text.len(), MAX_LOG_LINE_BYTES);
    assert!(lines[0].text.starts_with("aaaa"));
    assert_eq!(lines[1].text, "next");
}

#[test]
fn over_long_line_across_many_feeds_stays_bounded() {
    let mut splitter = LineSplitter::new();
    let mut out = Vec::new();
    for _ in 0..100 {
        splitter.feed(&[b'z'; 1000], &mut out);
    }
    splitter.feed(b"\nok\n", &mut out);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].text.len(), MAX_LOG_LINE_BYTES);
    assert_eq!(out[1].text, "ok");
}

#[test]
fn line_of_exactly_the_cap_is_not_truncated() {
    let mut input = vec![b'a'; MAX_LOG_LINE_BYTES];
    input.push(b'\n');
    let lines = split_all(&[&input]);
    assert_eq!(lines[0].text.len(), MAX_LOG_LINE_BYTES);
    assert!(!lines[0].text.contains('…'));
}

#[test]
fn truncation_never_splits_a_character() {
    let mut input = "é".repeat(MAX_LOG_LINE_BYTES).into_bytes();
    input.push(b'\n');
    let lines = split_all(&[&input]);
    assert!(lines[0].text.len() <= MAX_LOG_LINE_BYTES);
    assert!(lines[0].text.ends_with('…'));
    assert!(!lines[0].text.contains('\u{fffd}'));
}

#[test]
fn tabs_expanded_past_the_cap_are_still_bounded() {
    let mut input = vec![b'\t'; MAX_LOG_LINE_BYTES];
    input.push(b'\n');
    let lines = split_all(&[&input]);
    assert!(lines[0].text.len() <= MAX_LOG_LINE_BYTES);
    assert!(lines[0].text.ends_with('…'));
}

#[test]
fn invalid_bytes_expanding_past_the_cap_are_still_bounded() {
    let mut input = vec![0xff; MAX_LOG_LINE_BYTES];
    input.push(b'\n');
    let lines = split_all(&[&input]);
    assert!(lines[0].text.len() <= MAX_LOG_LINE_BYTES);
}

#[test]
fn strips_csi_sequences() {
    assert_eq!(texts(&[b"\x1b[31mred\x1b[0m plain \x1b[1;32;40mx\x1b[K\n"]), ["red plain x"]);
}

#[test]
fn strips_osc_sequences_with_bel_and_st() {
    assert_eq!(texts(&[b"a\x1b]0;title\x07b\x1b]8;;http://x\x1b\\c\n"]), ["abc"]);
}

#[test]
fn strips_escape_sequences_split_across_feeds() {
    assert_eq!(texts(&[b"a\x1b", b"[3", b"1mb\n"]), ["ab"]);
    assert_eq!(texts(&[b"a\x1b]0;ti", b"tle\x07b\n"]), ["ab"]);
    assert_eq!(texts(&[b"a\x1b]0;title\x1b", b"\\b\n"]), ["ab"]);
}

#[test]
fn strips_two_byte_and_charset_escapes() {
    assert_eq!(texts(&[b"a\x1bMb\x1b(Bc\n"]), ["abc"]);
}

#[test]
fn newline_ends_an_unterminated_escape() {
    assert_eq!(texts(&[b"a\x1b[31\nnext\n"]), ["a", "next"]);
    assert_eq!(texts(&[b"a\x1b]0;x\nnext\n"]), ["a", "next"]);
}

#[test]
fn a_line_that_is_only_an_unterminated_escape_is_not_emitted_at_finish() {
    assert!(texts(&[b"\x1b[31"]).is_empty());
}

#[test]
fn a_doubled_escape_still_strips_the_sequence() {
    assert_eq!(texts(&[b"a\x1b\x1b[31mb\n"]), ["ab"]);
}

#[test]
fn a_line_of_exactly_the_cap_ending_in_crlf_is_not_truncated() {
    let mut input = vec![b'a'; MAX_LOG_LINE_BYTES];
    input.extend_from_slice(b"\r\x07\r\n");
    let lines = split_all(&[&input]);
    assert_eq!(lines[0].text, "a".repeat(MAX_LOG_LINE_BYTES));
}

#[test]
fn visible_overflow_past_the_cap_is_still_truncated() {
    let mut input = vec![b'a'; MAX_LOG_LINE_BYTES];
    input.extend_from_slice(b"\rb\n");
    let lines = split_all(&[&input]);
    assert!(lines[0].text.ends_with('…'));
}

#[test]
fn removes_control_and_bidi_characters() {
    assert_eq!(texts(&["a\u{202e}b\u{200b}c\u{feff}d\u{2066}e\u{7}f\u{0}g\u{7f}h\r\n".as_bytes()]), ["abcdefgh"]);
    assert_eq!(texts(&[b"x\ry\x08z\n"]), ["xyz"]);
}

#[test]
fn tab_becomes_four_spaces() {
    assert_eq!(texts(&[b"a\tb\n"]), ["a    b"]);
}

#[test]
fn keeps_ordinary_unicode() {
    assert_eq!(texts(&["καλημέρα — 日本語 🙂\n".as_bytes()]), ["καλημέρα — 日本語 🙂"]);
}

#[test]
fn utf8_character_split_across_feeds_is_kept() {
    let bytes = "aé€🙂b\n".as_bytes();
    for cut in 1..bytes.len() {
        assert_eq!(texts(&[&bytes[..cut], &bytes[cut..]]), ["aé€🙂b"], "cut at {cut}");
    }
}

#[test]
fn invalid_bytes_are_replaced() {
    assert_eq!(texts(&[b"a\xffb\xc3\n"]), ["a\u{fffd}b\u{fffd}"]);
}

#[test]
fn lines_carry_their_level() {
    let lines = split_all(&[b"ERROR boom\nplain\n"]);
    assert_eq!(lines[0].level, LogLevel::Error);
    assert_eq!(lines[1].level, LogLevel::Unknown);
}

#[test]
fn level_is_detected_from_keywords() {
    assert_eq!(detect_level("ERROR something"), LogLevel::Error);
    assert_eq!(detect_level("error: nope"), LogLevel::Error);
    assert_eq!(detect_level("2026-11-27 [WARN] disk"), LogLevel::Warn);
    assert_eq!(detect_level("warning: x"), LogLevel::Warn);
    assert_eq!(detect_level("INFO started"), LogLevel::Info);
    assert_eq!(detect_level("debug: x"), LogLevel::Debug);
    assert_eq!(detect_level("level=Fatal msg=x"), LogLevel::Error);
    assert_eq!(detect_level("CRITICAL"), LogLevel::Error);
    assert_eq!(detect_level("trace x"), LogLevel::Debug);
    assert_eq!(detect_level("plain text"), LogLevel::Unknown);
    assert_eq!(detect_level(""), LogLevel::Unknown);
}

#[test]
fn level_matches_whole_words_only() {
    assert_eq!(detect_level("information about it"), LogLevel::Unknown);
    assert_eq!(detect_level("terror"), LogLevel::Unknown);
    assert_eq!(detect_level("errors happened"), LogLevel::Unknown);
    assert_eq!(detect_level("debugger attached"), LogLevel::Unknown);
}

#[test]
fn level_severity_order_wins() {
    assert_eq!(detect_level("info then error then debug"), LogLevel::Error);
    assert_eq!(detect_level("debug info warn"), LogLevel::Warn);
    assert_eq!(detect_level("debug info"), LogLevel::Info);
}

#[test]
fn level_reads_a_syslog_priority_prefix() {
    assert_eq!(detect_level("<0>x"), LogLevel::Error);
    assert_eq!(detect_level("<3>started"), LogLevel::Error);
    assert_eq!(detect_level("<4>x"), LogLevel::Warn);
    assert_eq!(detect_level("<5>x"), LogLevel::Info);
    assert_eq!(detect_level("<6>x"), LogLevel::Info);
    assert_eq!(detect_level("<7>x"), LogLevel::Debug);
    assert_eq!(detect_level("<>x"), LogLevel::Unknown);
}

#[test]
fn level_reads_facility_and_severity_from_a_syslog_priority() {
    assert_eq!(detect_level("<34>x"), LogLevel::Error);
    assert_eq!(detect_level("<165>x"), LogLevel::Info);
    assert_eq!(detect_level("<191>x"), LogLevel::Debug);
    assert_eq!(detect_level("<12>x"), LogLevel::Warn);
    assert_eq!(detect_level("<8>x"), LogLevel::Error);
}

#[test]
fn level_ignores_a_malformed_syslog_priority() {
    assert_eq!(detect_level("<+3>x"), LogLevel::Unknown);
    assert_eq!(detect_level("<192>x"), LogLevel::Unknown);
    assert_eq!(detect_level("<-3>x"), LogLevel::Unknown);
    assert_eq!(detect_level("<0003>x"), LogLevel::Unknown);
    assert_eq!(detect_level("<192>warning x"), LogLevel::Warn);
}

#[test]
fn failed_and_fail_are_errors_as_whole_words() {
    assert_eq!(detect_level("Failed to start apport.service"), LogLevel::Error);
    assert_eq!(detect_level("auth fail for root"), LogLevel::Error);
    assert_eq!(detect_level("failover complete"), LogLevel::Unknown);
    assert_eq!(detect_level("failures counted"), LogLevel::Unknown);
    assert_eq!(detect_level("opened /var/log/err.log"), LogLevel::Error);
}

#[test]
fn level_only_looks_at_the_first_200_characters() {
    let within = format!("{}error", "a ".repeat(97));
    assert_eq!(detect_level(&within), LogLevel::Error);
    let beyond = format!("{}error", "a ".repeat(100));
    assert_eq!(detect_level(&beyond), LogLevel::Unknown);
    let multibyte = format!("{}error", "é ".repeat(100));
    assert_eq!(detect_level(&multibyte), LogLevel::Unknown);
}

const TEN_MIB: usize = 10 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;

fn endless_line(head: &[u8], filler: u8) -> (usize, Vec<LogLine>) {
    let mut splitter = LineSplitter::new();
    let mut out = Vec::new();
    let mut peak = 0;
    splitter.feed(head, &mut out);
    let chunk = vec![filler; CHUNK];
    for _ in 0..TEN_MIB / CHUNK {
        splitter.feed(&chunk, &mut out);
        peak = peak.max(splitter.buffered_capacity());
    }
    assert!(out.is_empty());
    splitter.feed(b"\nnext line\n", &mut out);
    splitter.finish(&mut out);
    (peak, out)
}

fn assert_one_truncated_line_then_next(lines: &[LogLine]) {
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].text.ends_with('…'));
    assert!(lines[0].text.len() <= MAX_LOG_LINE_BYTES);
    assert_eq!(lines[1].text, "next line");
}

#[test]
fn ten_mib_line_without_newline_stays_bounded_and_truncates() {
    let (peak, lines) = endless_line(b"", b'x');
    assert!(peak <= 2 * MAX_LOG_LINE_BYTES, "buffer grew to {peak}");
    assert_one_truncated_line_then_next(&lines);
}

#[test]
fn ten_mib_after_a_long_head_and_an_unterminated_osc_stays_bounded_and_truncates() {
    let mut head = vec![b'h'; MAX_LOG_LINE_BYTES + 10];
    head.extend_from_slice(b"\x1b]0;");
    let (peak, lines) = endless_line(&head, b'x');
    assert!(peak <= 2 * MAX_LOG_LINE_BYTES, "buffer grew to {peak}");
    assert_one_truncated_line_then_next(&lines);
}

#[test]
fn ten_mib_after_a_long_head_and_an_unterminated_csi_stays_bounded_and_truncates() {
    let mut head = vec![b'h'; MAX_LOG_LINE_BYTES + 10];
    head.extend_from_slice(b"\x1b[");
    let (peak, lines) = endless_line(&head, b';');
    assert!(peak <= 2 * MAX_LOG_LINE_BYTES, "buffer grew to {peak}");
    assert_one_truncated_line_then_next(&lines);
}

#[test]
fn ten_mib_inside_a_leading_unterminated_osc_buffers_nothing() {
    let (peak, lines) = endless_line(b"\x1b]0;", b'x');
    assert_eq!(peak, 0);
    assert_eq!(lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>(), ["", "next line"]);
}

#[test]
fn ten_mib_inside_a_leading_unterminated_csi_buffers_nothing() {
    let (peak, lines) = endless_line(b"\x1b[", b';');
    assert_eq!(peak, 0);
    assert_eq!(lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>(), ["", "next line"]);
}
