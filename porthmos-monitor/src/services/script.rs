use super::unit::UnitName;
use crate::probe::{PRELUDE, Token, with_token};

const BODY: &str = r#"m uid
id -u 2>/dev/null
m systemd
if [ -d /run/systemd/system ] && command -v systemctl >/dev/null 2>&1; then echo yes; fi
m units
systemctl list-units --type=service --all --plain --no-legend --no-pager --full 2>/dev/null | head -n 2001
m files
systemctl list-unit-files --type=service --plain --no-legend --no-pager 2>/dev/null | head -n 4001
m end
"#;

pub fn services_command(token: &Token) -> String {
    with_token(&format!("{PRELUDE}{BODY}"), token)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceVerb {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}

impl ServiceVerb {
    fn word(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    Direct,
    Sudo,
}

impl Privilege {
    pub fn for_uid(uid: u32) -> Self {
        if uid == 0 { Self::Direct } else { Self::Sudo }
    }
}

pub fn service_command(verb: ServiceVerb, unit: &UnitName, privilege: Privilege) -> String {
    let prefix = match privilege {
        Privilege::Direct => "",
        Privilege::Sudo => "sudo -n ",
    };
    format!("{prefix}systemctl {} {} </dev/null", verb.word(), unit.as_str())
}

pub fn status_command(unit: &UnitName) -> String {
    format!("systemctl status --no-pager --lines=20 {} </dev/null", unit.as_str())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoProblem {
    NeedsPassword,
    NotAllowed,
    NotInstalled,
}

pub fn sudo_problem(stderr: &str) -> Option<SudoProblem> {
    let text = stderr.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));
    if has(&["a password is required"]) {
        Some(SudoProblem::NeedsPassword)
    } else if has(&["is not in the sudoers", "may not run sudo"]) {
        Some(SudoProblem::NotAllowed)
    } else if has(&["sudo: not found", "sudo: command not found"]) {
        Some(SudoProblem::NotInstalled)
    } else {
        None
    }
}
