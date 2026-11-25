use std::collections::HashMap;

use crate::{
    Interface,
    parse::split::{Name, Split},
    rate::per_second,
};

const MAX_INTERFACES: usize = 64;
const NAME_LIMIT: usize = 32;
const TRANSMIT_FIELD: usize = 8;

pub(crate) type Counters = HashMap<String, (u64, u64)>;

pub(crate) fn counters(split: &Split) -> Option<Counters> {
    let mut found = Counters::new();
    for line in split.lines(Name::Netdev)? {
        let text = String::from_utf8_lossy(line);
        let Some((name, rest)) = text.split_once(':') else { continue };
        let name = super::text(name.as_bytes(), NAME_LIMIT);
        let numbers: Vec<Option<u64>> = rest.split_whitespace().map(|field| field.parse().ok()).collect();
        let (Some(name), Some(Some(received)), Some(Some(sent))) = (name, numbers.first(), numbers.get(TRANSMIT_FIELD))
        else {
            continue;
        };
        if found.len() < MAX_INTERFACES {
            found.entry(name).or_insert((*received, *sent));
        }
    }
    (!found.is_empty()).then_some(found)
}

pub(crate) fn interfaces(now: &Counters, before: &Counters, elapsed: std::time::Duration) -> Vec<Interface> {
    let mut listed: Vec<Interface> = now
        .iter()
        .map(|(name, (received, sent))| {
            let earlier = before.get(name);
            Interface {
                name: name.clone(),
                rx_per_second: earlier.and_then(|(was, _)| per_second(*received, *was, elapsed)),
                tx_per_second: earlier.and_then(|(_, was)| per_second(*sent, *was, elapsed)),
            }
        })
        .collect();
    listed.sort_by(|left, right| (left.name == "lo", &left.name).cmp(&(right.name == "lo", &right.name)));
    listed
}
