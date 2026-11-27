use std::collections::HashMap;

use super::NetInterface;
use crate::parse::{
    sections,
    split::{Name, Split},
};

const NAME_LIMIT: usize = 32;
const ADDRESS_LIMIT: usize = 64;

#[derive(Default)]
struct Details {
    up: Option<bool>,
    mac: Option<String>,
    mtu: Option<u32>,
}

fn details(split: &Split) -> HashMap<String, Details> {
    let mut found = HashMap::new();
    for line in split.lines(Name::SysNet).unwrap_or_default() {
        let text = String::from_utf8_lossy(line);
        let mut fields = text.splitn(4, '|');
        let (Some(name), Some(state), Some(mac), Some(mtu)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some(name) = sections::text(name.as_bytes(), NAME_LIMIT) else { continue };
        let up = match state.trim() {
            "up" => Some(true),
            "down" | "lowerlayerdown" | "notpresent" => Some(false),
            _ => None,
        };
        found.entry(name).or_insert(Details {
            up,
            mac: sections::text(mac.as_bytes(), NAME_LIMIT),
            mtu: mtu.trim().parse().ok(),
        });
    }
    found
}

fn addresses(split: &Split) -> HashMap<String, Vec<String>> {
    let mut found: HashMap<String, Vec<String>> = HashMap::new();
    for line in split.lines(Name::IpAddr).unwrap_or_default() {
        let text = String::from_utf8_lossy(line);
        let fields: Vec<&str> = text.split_whitespace().take(4).collect();
        let [_, name, "inet" | "inet6", address] = fields[..] else { continue };
        let name = name.split('@').next().unwrap_or(name);
        if let (Some(name), Some(address)) =
            (sections::text(name.as_bytes(), NAME_LIMIT), sections::text(address.as_bytes(), ADDRESS_LIMIT))
        {
            found.entry(name).or_default().push(address);
        }
    }
    found
}

pub(super) fn interfaces(split: &Split) -> Option<Vec<NetInterface>> {
    let counters = sections::counters(split)?;
    let mut details = details(split);
    let mut addresses = addresses(split);
    let mut listed: Vec<NetInterface> = counters
        .into_iter()
        .map(|(name, (rx_bytes, tx_bytes))| {
            let known = details.remove(&name).unwrap_or_default();
            NetInterface {
                up: known.up,
                addresses: addresses.remove(&name).unwrap_or_default(),
                mac: known.mac,
                mtu: known.mtu,
                rx_bytes,
                tx_bytes,
                name,
            }
        })
        .collect();
    listed.sort_by(|left, right| (left.name == "lo", &left.name).cmp(&(right.name == "lo", &right.name)));
    Some(listed)
}
