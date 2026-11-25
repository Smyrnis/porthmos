use std::time::Duration;

use crate::{Cpu, Section, Snapshot, probe::Token};

mod sections;
mod split;

use sections::{Counters, CpuTicks, History, Inputs};
use split::Name;

#[derive(Debug, Clone, Default)]
pub struct Sample {
    cpu: Option<CpuTicks>,
    network: Counters,
    processes: History,
}

#[derive(Debug)]
pub enum Parsed {
    Linux { snapshot: Box<Snapshot>, sample: Sample },
    Unsupported { os: String },
    Garbled,
}

pub fn parse(output: &[u8], token: &Token, previous: Option<&Sample>, elapsed: Duration) -> Parsed {
    let split = split::split(output, token);
    let Some(os) = split.first_line(Name::Os).and_then(|line| sections::text(line, 64)) else {
        return Parsed::Garbled;
    };
    if os != "Linux" {
        return Parsed::Unsupported { os };
    }
    let mut problems = Vec::new();
    let mut note = |found: bool, section: Section| {
        if !found {
            problems.push(section);
        }
    };

    let host = sections::host(&split);
    note(host.is_some(), Section::Host);
    let load = sections::load(&split);
    note(load.is_some(), Section::Load);
    let memory = sections::memory(&split);
    note(memory.is_some(), Section::Memory);
    let reading = sections::cpu(&split);
    note(reading.is_some(), Section::Cpu);

    let ticks = reading.as_ref().and_then(|reading| reading.ticks);
    let cores = reading.as_ref().and_then(|reading| reading.cores);
    let before_cpu = previous.and_then(|sample| sample.cpu);
    let percent = ticks.zip(before_cpu).and_then(|(now, before)| sections::percent(now, before));
    let cpu_delta = ticks.zip(before_cpu).and_then(|(now, before)| now.total.checked_sub(before.total));

    let disks = sections::disks(&split);
    note(disks.is_some(), Section::Disks);
    let counters = sections::counters(&split);
    note(counters.is_some(), Section::Network);
    let no_counters = Counters::new();
    let interfaces = counters
        .as_ref()
        .map(|now| sections::interfaces(now, previous.map_or(&no_counters, |sample| &sample.network), elapsed))
        .unwrap_or_default();
    let inputs = Inputs {
        memory_total: memory.map(|memory| memory.total),
        cores,
        cpu_delta,
        before: previous.map(|sample| &sample.processes),
    };
    let found = sections::processes(&split, &inputs);
    note(found.is_some(), Section::Processes);
    let (processes, history) = found.unwrap_or_default();
    let uid = split
        .first_line(Name::Uid)
        .and_then(|line| String::from_utf8_lossy(line).trim().parse::<u32>().ok())
        .unwrap_or(u32::MAX);
    let snapshot = Snapshot {
        host: host.unwrap_or_default(),
        cpu: Cpu { cores, percent },
        load,
        memory,
        disks: disks.unwrap_or_default(),
        interfaces,
        processes,
        uid,
        problems,
    };
    Parsed::Linux {
        snapshot: Box::new(snapshot),
        sample: Sample { cpu: ticks, network: counters.unwrap_or_default(), processes: history },
    }
}

#[cfg(test)]
mod tests;
