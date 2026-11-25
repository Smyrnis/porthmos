use crate::parse::split::{Name, Split};

const MAX_CORES: u32 = 4096;
const TICK_FIELDS: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CpuTicks {
    pub(crate) total: u64,
    pub(crate) idle: u64,
}

pub(crate) struct CpuReading {
    pub(crate) cores: Option<u32>,
    pub(crate) ticks: Option<CpuTicks>,
}

pub(crate) fn cpu(split: &Split) -> Option<CpuReading> {
    let lines = split.lines(Name::Stat)?;
    let mut ticks = None;
    let mut cores = 0u32;
    for line in lines {
        let text = String::from_utf8_lossy(line);
        let mut fields = text.split_whitespace();
        let Some(label) = fields.next() else { continue };
        if label == "cpu" {
            let numbers: Vec<u64> = fields.take(TICK_FIELDS).map_while(|field| field.parse().ok()).collect();
            if numbers.len() >= 4 {
                let total = numbers.iter().fold(0u64, |sum, value| sum.saturating_add(*value));
                let idle = numbers[3].saturating_add(numbers.get(4).copied().unwrap_or(0));
                ticks = Some(CpuTicks { total, idle });
            }
        } else if label
            .strip_prefix("cpu")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
        {
            cores = (cores + 1).min(MAX_CORES);
        }
    }
    (ticks.is_some() || cores > 0).then_some(CpuReading { cores: (cores > 0).then_some(cores), ticks })
}

pub(crate) fn percent(now: CpuTicks, before: CpuTicks) -> Option<f32> {
    let total = now.total.checked_sub(before.total).filter(|delta| *delta > 0)?;
    let idle = now.idle.checked_sub(before.idle)?;
    let busy = total.saturating_sub(idle);
    Some(((busy as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32)
}
