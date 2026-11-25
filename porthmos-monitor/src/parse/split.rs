use std::collections::HashMap;

use crate::{MAX_LINE_BYTES, MAX_OUTPUT_BYTES, probe::Token};

const MAX_LINES_PER_SECTION: usize = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Name {
    Os,
    Uid,
    Hostname,
    OsRelease,
    Kernel,
    Uptime,
    Loadavg,
    Meminfo,
    Stat,
    Netdev,
    Df,
    Passwd,
    Pagesize,
    Status,
    Cmdline,
    Procs,
    End,
}

impl Name {
    fn from_text(text: &[u8]) -> Option<Self> {
        Some(match text {
            b"os" => Self::Os,
            b"uid" => Self::Uid,
            b"hostname" => Self::Hostname,
            b"osrelease" => Self::OsRelease,
            b"kernel" => Self::Kernel,
            b"uptime" => Self::Uptime,
            b"loadavg" => Self::Loadavg,
            b"meminfo" => Self::Meminfo,
            b"stat" => Self::Stat,
            b"netdev" => Self::Netdev,
            b"df" => Self::Df,
            b"passwd" => Self::Passwd,
            b"pagesize" => Self::Pagesize,
            b"status" => Self::Status,
            b"cmdline" => Self::Cmdline,
            b"procs" => Self::Procs,
            b"end" => Self::End,
            _ => return None,
        })
    }
}

pub(crate) struct Split<'a> {
    sections: HashMap<Name, Vec<&'a [u8]>>,
}

impl<'a> Split<'a> {
    pub(crate) fn lines(&self, name: Name) -> Option<&[&'a [u8]]> {
        self.sections.get(&name).map(Vec::as_slice)
    }

    pub(crate) fn first_line(&self, name: Name) -> Option<&'a [u8]> {
        self.lines(name).and_then(|lines| lines.iter().copied().find(|line| !line.iter().all(u8::is_ascii_whitespace)))
    }
}

pub(crate) fn split<'a>(output: &'a [u8], token: &Token) -> Split<'a> {
    let output = &output[..output.len().min(MAX_OUTPUT_BYTES)];
    let prefix = format!("@@{}:", token.as_str()).into_bytes();
    let mut sections: HashMap<Name, Vec<&'a [u8]>> = HashMap::new();
    let mut current: Option<Name> = None;
    let mut complete = false;
    let mut last_opened: Option<Name> = None;
    for raw in output.split(|byte| *byte == b'\n') {
        let raw = &raw[..raw.len().min(MAX_LINE_BYTES)];
        let line = raw.strip_suffix(b"\r").unwrap_or(raw);
        if let Some(rest) = line.strip_prefix(prefix.as_slice()) {
            match Name::from_text(rest) {
                Some(Name::End) => {
                    complete = true;
                    break;
                }
                Some(name) if !sections.contains_key(&name) => {
                    sections.insert(name, Vec::new());
                    current = Some(name);
                    last_opened = Some(name);
                    continue;
                }
                Some(_) => {
                    current = None;
                    continue;
                }
                None => {}
            }
        }
        if let Some(lines) = current.and_then(|name| sections.get_mut(&name))
            && lines.len() < MAX_LINES_PER_SECTION
        {
            lines.push(line);
        }
    }
    if !complete && let Some(name) = last_opened {
        sections.remove(&name);
    }
    Split { sections }
}
