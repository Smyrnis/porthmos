use std::{cmp::Ordering, collections::HashMap};

use crate::{
    MAX_PROCESSES, Process, UNKNOWN_UID,
    parse::split::{Name, Split},
};

const NAME_LIMIT: usize = 64;
const COMMAND_LIMIT: usize = 512;
const COMMAND_SCRATCH: usize = 2048;
const DEFAULT_PAGE_SIZE: u64 = 4096;
const MIN_STAT_FIELDS: usize = 22;
const UTIME: usize = 11;
const STIME: usize = 12;
const NICE: usize = 16;
const START_TIME: usize = 19;
const RESIDENT_PAGES: usize = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ticks {
    pub(crate) start: u64,
    pub(crate) used: u64,
}

pub(crate) type History = HashMap<u32, Ticks>;

pub(crate) struct Inputs<'a> {
    pub(crate) memory_total: Option<u64>,
    pub(crate) cores: Option<u32>,
    pub(crate) cpu_delta: Option<u64>,
    pub(crate) before: Option<&'a History>,
}

struct Raw {
    process: Process,
    used: u64,
}

fn users(split: &Split) -> HashMap<u32, String> {
    let mut names = HashMap::new();
    for line in split.lines(Name::Passwd).unwrap_or_default() {
        let text = String::from_utf8_lossy(line);
        let Some((name, uid)) = text.rsplit_once(':') else { continue };
        if let (Ok(uid), Some(name)) = (uid.trim().parse::<u32>(), super::text(name.as_bytes(), NAME_LIMIT)) {
            names.entry(uid).or_insert(name);
        }
    }
    names
}

fn pid_of(path: &str, suffix: &str) -> Option<u32> {
    path.strip_prefix("/proc/")?.strip_suffix(suffix)?.parse().ok()
}

fn attributed<'a>(line: &'a str, file: &str) -> Option<(u32, &'a str)> {
    let (pid, rest) = line.strip_prefix("/proc/")?.split_once('/')?;
    let content = rest.strip_prefix(file)?.strip_prefix(':')?;
    Some((pid.parse().ok()?, content))
}

fn uids(split: &Split) -> HashMap<u32, u32> {
    let mut found = HashMap::new();
    for line in split.lines(Name::Status).unwrap_or_default() {
        let text = String::from_utf8_lossy(line);
        let Some((path, rest)) = text.split_once(":Uid:") else { continue };
        let uid = rest.split_whitespace().next().and_then(|field| field.parse::<u32>().ok());
        if let (Some(pid), Some(uid)) = (pid_of(path, "/status"), uid) {
            found.entry(pid).or_insert(uid);
        }
    }
    found
}

fn commands(split: &Split) -> HashMap<u32, String> {
    let mut found: HashMap<u32, String> = HashMap::new();
    for line in split.lines(Name::Cmdline).unwrap_or_default() {
        let text = String::from_utf8_lossy(line);
        let Some((pid, content)) = attributed(&text, "cmdline") else { continue };
        let entry = found.entry(pid).or_default();
        if entry.len() < COMMAND_SCRATCH {
            for word in content.split_whitespace() {
                entry.push(' ');
                entry.push_str(word);
            }
        }
    }
    found
}

fn percent(inputs: &Inputs, pid: u32, now: Ticks) -> Option<f32> {
    let before = inputs.before?.get(&pid).filter(|before| before.start == now.start)?;
    let used = now.used.checked_sub(before.used)?;
    let delta = inputs.cpu_delta.filter(|delta| *delta > 0)?;
    let cores = f64::from(inputs.cores?);
    Some(((used as f64 / delta as f64) * cores * 100.0).clamp(0.0, cores * 100.0) as f32)
}

fn rank(left: &Raw, right: &Raw, by_memory: bool) -> Ordering {
    let value = |raw: &Raw| if by_memory { raw.process.memory_percent } else { raw.process.cpu_percent.unwrap_or(0.0) };
    value(right).total_cmp(&value(left)).then(right.used.cmp(&left.used)).then(left.process.pid.cmp(&right.process.pid))
}

pub(crate) fn processes(split: &Split, inputs: &Inputs) -> Option<(Vec<Process>, History)> {
    let stats = split.lines(Name::Procs)?;
    let (uids, commands, users) = (uids(split), commands(split), users(split));
    let page = split
        .first_line(Name::Pagesize)
        .and_then(|line| String::from_utf8_lossy(line).trim().parse::<u64>().ok())
        .filter(|size| *size > 0)
        .unwrap_or(DEFAULT_PAGE_SIZE);
    let mut history = History::new();
    let mut raws: Vec<Raw> = Vec::new();
    for line in stats {
        let line = String::from_utf8_lossy(line);
        let Some((owner, text)) = attributed(&line, "stat") else { continue };
        let (Some(open), Some(close)) = (text.find('('), text.rfind(')')) else { continue };
        if close < open {
            continue;
        }
        let Ok(pid) = text[..open].trim().parse::<u32>() else { continue };
        if pid != owner {
            continue;
        }
        let fields: Vec<&str> = text[close + 1..].split_whitespace().collect();
        if fields.len() < MIN_STAT_FIELDS || history.contains_key(&pid) {
            continue;
        }
        let number = |index: usize| fields[index].parse::<u64>().ok();
        let (Some(utime), Some(stime), Some(start), Ok(nice), Ok(resident)) = (
            number(UTIME),
            number(STIME),
            number(START_TIME),
            fields[NICE].parse::<i32>(),
            fields[RESIDENT_PAGES].parse::<i64>(),
        ) else {
            continue;
        };
        let ticks = Ticks { start, used: utime.saturating_add(stime) };
        history.insert(pid, ticks);
        let name = super::text(&text.as_bytes()[open + 1..close], NAME_LIMIT).unwrap_or_else(|| "?".to_string());
        let uid = uids.get(&pid).copied().unwrap_or(UNKNOWN_UID);
        let command = commands
            .get(&pid)
            .and_then(|raw| super::text(raw.as_bytes(), COMMAND_LIMIT))
            .unwrap_or_else(|| format!("[{name}]"));
        let memory_percent = inputs
            .memory_total
            .filter(|total| *total > 0)
            .map(|total| (resident.max(0) as f64 * page as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32)
            .unwrap_or(0.0);
        raws.push(Raw {
            process: Process {
                pid,
                uid,
                user: users.get(&uid).cloned(),
                name,
                command,
                cpu_percent: percent(inputs, pid, ticks),
                memory_percent,
                nice,
            },
            used: ticks.used,
        });
    }
    if raws.is_empty() {
        return None;
    }
    let mut keep: Vec<bool> = vec![false; raws.len()];
    for by_memory in [false, true] {
        let mut order: Vec<usize> = (0..raws.len()).collect();
        order.sort_by(|left, right| rank(&raws[*left], &raws[*right], by_memory));
        for index in order.into_iter().take(MAX_PROCESSES) {
            keep[index] = true;
        }
    }
    let mut kept: Vec<Process> =
        raws.into_iter().zip(keep).filter_map(|(raw, keep)| keep.then_some(raw.process)).collect();
    kept.sort_by_key(|process| process.pid);
    Some((kept, history))
}
