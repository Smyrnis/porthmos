use crate::{
    UNKNOWN_UID,
    parse::{
        sections,
        split::{self, Name},
    },
    probe::Token,
};

mod parse;
mod script;
mod unit;

pub use script::{
    Privilege, ServiceVerb, SudoProblem, service_command, services_command, status_command, sudo_problem,
};
pub use unit::{ActiveState, BootState, StatusFilter, Unit, UnitName, arranged_units, verb_allowed};

pub const MAX_UNITS: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicesSnapshot {
    pub units: Vec<Unit>,
    pub uid: u32,
}

#[derive(Debug)]
pub enum ParsedServices {
    Linux(Box<ServicesSnapshot>),
    NoSystemd,
    Unsupported { os: String },
    Garbled,
}

pub fn parse_services(output: &[u8], token: &Token) -> ParsedServices {
    let split = split::split(output, token);
    let Some(os) = split.first_line(Name::Os).and_then(|line| sections::text(line, 64)) else {
        return ParsedServices::Garbled;
    };
    if os != "Linux" {
        return ParsedServices::Unsupported { os };
    }
    if split.lines(Name::Systemd).is_none() {
        return ParsedServices::Garbled;
    }
    if split.first_line(Name::Systemd).map(<[u8]>::trim_ascii) != Some(b"yes".as_slice()) {
        return ParsedServices::NoSystemd;
    }
    let Some(listed) = split.lines(Name::Units) else {
        return ParsedServices::Garbled;
    };
    let files = split.lines(Name::Files).unwrap_or_default();
    let uid = split
        .first_line(Name::Uid)
        .and_then(|line| String::from_utf8_lossy(line).trim().parse::<u32>().ok())
        .unwrap_or(UNKNOWN_UID);
    ParsedServices::Linux(Box::new(ServicesSnapshot { units: parse::units(listed, files), uid }))
}

#[cfg(test)]
mod tests;
