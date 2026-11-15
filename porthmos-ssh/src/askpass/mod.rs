use std::{
    ffi::OsString,
    io::{self, Write},
};

pub const HELPER_SWITCH: &str = "PORTHMOS_ASKPASS";
pub const PASSWORD_VARIABLE: &str = "PORTHMOS_ASKPASS_PASSWORD";
pub const USER_VARIABLE: &str = "PORTHMOS_ASKPASS_USER";
pub const MAX_PASSWORD_BYTES: usize = 1023;

const PASSWORD_SUFFIX: &str = "'s password: ";
const INTERACTIVE_SUFFIX: &str = ") Password: ";

pub fn usable_password(password: &str) -> bool {
    !password.is_empty() && password.len() <= MAX_PASSWORD_BYTES && !password.contains('\n') && !password.contains('\0')
}

fn plain_host(host: &str) -> bool {
    !host.is_empty()
        && !host.chars().any(|character| character.is_whitespace() || character.is_control() || character == '@')
}

fn password_prompt_host<'a>(prompt: &'a str, user: &str) -> Option<&'a str> {
    prompt.strip_prefix(user)?.strip_prefix('@')?.strip_suffix(PASSWORD_SUFFIX)
}

fn interactive_prompt_host<'a>(prompt: &'a str, user: &str) -> Option<&'a str> {
    prompt.strip_prefix('(')?.strip_prefix(user)?.strip_prefix('@')?.strip_suffix(INTERACTIVE_SUFFIX)
}

pub fn answer_for(prompt: &str, user: &str, password: &str) -> Option<String> {
    if user.is_empty() {
        return None;
    }
    let host = password_prompt_host(prompt, user).or_else(|| interactive_prompt_host(prompt, user))?;
    plain_host(host).then(|| password.to_string())
}

pub fn run_helper(arguments: &[OsString], lookup: impl Fn(&str) -> Option<String>, out: &mut impl Write) -> i32 {
    let Some(prompt) = arguments.first().and_then(|argument| argument.to_str()) else {
        return 1;
    };
    let (Some(user), Some(password)) = (lookup(USER_VARIABLE), lookup(PASSWORD_VARIABLE)) else {
        return 1;
    };
    let Some(answer) = answer_for(prompt, &user, &password) else {
        return 1;
    };
    match writeln!(out, "{answer}").and_then(|()| out.flush()) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

pub fn run_if_requested() -> Option<i32> {
    if std::env::var(HELPER_SWITCH).as_deref() != Ok("1") {
        return None;
    }
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut stdout = io::stdout().lock();
    Some(run_helper(&arguments, |name| std::env::var(name).ok(), &mut stdout))
}

#[cfg(test)]
mod tests;
