use super::{finite, text};
use crate::{
    Host, Load,
    parse::split::{Name, Split},
};

const NAME_LIMIT: usize = 255;

fn os_name(split: &Split) -> Option<String> {
    let lines = split.lines(Name::OsRelease)?;
    let find = |key: &str| {
        lines.iter().find_map(|line| {
            let value = String::from_utf8_lossy(line.strip_prefix(key.as_bytes())?).into_owned();
            text(value.trim().trim_matches(['"', '\'']).as_bytes(), NAME_LIMIT)
        })
    };
    find("PRETTY_NAME=").or_else(|| find("NAME="))
}

pub(crate) fn host(split: &Split) -> Option<Host> {
    let first = |name: Name| split.first_line(name).and_then(|line| text(line, NAME_LIMIT));
    let uptime = split.first_line(Name::Uptime).and_then(|line| {
        let first = String::from_utf8_lossy(line).split_whitespace().next()?.to_string();
        let seconds = first.parse::<f64>().ok().filter(|value| value.is_finite() && *value >= 0.0)?;
        Some(seconds as u64)
    });
    let host = Host {
        hostname: first(Name::Hostname),
        os: os_name(split),
        kernel: first(Name::Kernel),
        uptime_seconds: uptime,
    };
    let empty = host.hostname.is_none() && host.os.is_none() && host.kernel.is_none() && host.uptime_seconds.is_none();
    (!empty).then_some(host)
}

pub(crate) fn load(split: &Split) -> Option<Load> {
    let line = String::from_utf8_lossy(split.first_line(Name::Loadavg)?).into_owned();
    let mut numbers = line.split_whitespace().take(3).map(finite);
    Some(Load { one: numbers.next()??, five: numbers.next()??, fifteen: numbers.next()?? })
}
