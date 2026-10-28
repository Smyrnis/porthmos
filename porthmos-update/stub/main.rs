use std::{
    env, fs,
    io::{self, Read},
};

const MARKER: &[u8] = b"\nPORTHMOS-STUB-VERSION:";

fn version_in(bytes: &[u8]) -> Option<String> {
    let start = bytes.windows(MARKER.len()).rposition(|window| window == MARKER)? + MARKER.len();
    Some(String::from_utf8_lossy(&bytes[start..]).trim().to_string())
}

fn hold_until_stdin_closes() {
    let mut buffer = [0u8; 64];
    while io::stdin().read(&mut buffer).is_ok_and(|read| read > 0) {}
}

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--hold") {
        hold_until_stdin_closes();
    } else if arguments.iter().any(|argument| argument == "--version") {
        let bytes = env::current_exe().and_then(fs::read).unwrap_or_default();
        println!("porthmos {}", version_in(&bytes).unwrap_or_default());
    }
}
