use crate::{
    Disk,
    parse::split::{Name, Split},
};

const KIBIBYTE: u64 = 1024;
const MAX_DISKS: usize = 64;
const HIDDEN_DEVICES: [&str; 5] = ["tmpfs", "devtmpfs", "udev", "ramfs", "shm"];
const HIDDEN_MOUNTS: [&str; 4] = ["/proc", "/sys", "/dev", "/run"];

fn hidden(device: &str, mount: &str) -> bool {
    HIDDEN_DEVICES.contains(&device)
        || HIDDEN_MOUNTS
            .iter()
            .any(|root| mount == *root || mount.strip_prefix(root).is_some_and(|rest| rest.starts_with('/')))
}

fn row(tokens: &[&str]) -> Option<Disk> {
    let [device, total, used, _available, capacity, mount @ ..] = tokens else { return None };
    if mount.is_empty() || !(capacity.ends_with('%') || *capacity == "-") {
        return None;
    }
    let total = total.parse::<u64>().ok()?.saturating_mul(KIBIBYTE);
    let used = used.parse::<u64>().ok()?.saturating_mul(KIBIBYTE);
    let mount = mount.join(" ");
    (!hidden(device, &mount)).then(|| Disk { mount, device: (*device).to_string(), total, used })
}

pub(crate) fn disks(split: &Split) -> Option<Vec<Disk>> {
    let mut found = Vec::new();
    let mut pending: Option<String> = None;
    for line in split.lines(Name::Df)? {
        let text = String::from_utf8_lossy(line);
        let wrapped =
            text.split_whitespace().next().is_some_and(|first| first.bytes().all(|byte| byte.is_ascii_digit()));
        let joined = match pending.take() {
            Some(device) if wrapped => format!("{device} {text}"),
            _ => text.into_owned(),
        };
        let tokens: Vec<&str> = joined.split_whitespace().collect();
        if tokens.is_empty() || tokens[0] == "Filesystem" {
            continue;
        }
        if tokens.len() == 1 {
            pending = Some(tokens[0].to_string());
        } else if let Some(disk) = row(&tokens) {
            found.push(disk);
        }
    }
    found.truncate(MAX_DISKS);
    (!found.is_empty()).then_some(found)
}
