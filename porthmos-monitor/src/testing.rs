pub(crate) struct ProcSpec {
    pub pid: u32,
    pub uid: Option<u32>,
    pub comm: &'static str,
    pub ticks: u64,
    pub nice: i32,
    pub rss_pages: u64,
    pub cmdline: String,
}

impl ProcSpec {
    pub(crate) fn new(pid: u32, comm: &'static str) -> Self {
        Self { pid, uid: Some(1000), comm, ticks: 100, nice: 0, rss_pages: 1000, cmdline: comm.to_string() }
    }
}

pub(crate) fn stat_line(spec: &ProcSpec) -> String {
    let utime = spec.ticks / 2;
    let stime = spec.ticks - utime;
    let fields = [
        "S".to_string(),
        "1".to_string(),
        spec.pid.to_string(),
        spec.pid.to_string(),
        "0".into(),
        "-1".into(),
        "4194560".into(),
        "1000".into(),
        "0".into(),
        "0".into(),
        "0".into(),
        utime.to_string(),
        stime.to_string(),
        "0".into(),
        "0".into(),
        "20".into(),
        spec.nice.to_string(),
        "1".into(),
        "0".into(),
        "5000".into(),
        "100000000".into(),
        spec.rss_pages.to_string(),
    ];
    format!("{} ({}) {} 18446744073709551615 1 1 0 0 0 0", spec.pid, spec.comm, fields.join(" "))
}

pub(crate) fn with_processes(base: &str, specs: &[ProcSpec]) -> String {
    let head = &base[..base.find("@@T:status").expect("the fixture has a status section")];
    let mut text = head.to_string();
    text.push_str("@@T:status\n");
    for spec in specs {
        if let Some(uid) = spec.uid {
            text.push_str(&format!("/proc/{}/status:Uid:\t{uid}\t{uid}\t{uid}\t{uid}\n", spec.pid));
        }
    }
    text.push_str("@@T:cmdline\n");
    for spec in specs {
        for line in spec.cmdline.split('\n').filter(|line| !line.is_empty()) {
            text.push_str(&format!("/proc/{}/cmdline:{line}\n", spec.pid));
        }
    }
    text.push_str("@@T:procs\n");
    for spec in specs {
        for line in stat_line(spec).split('\n') {
            text.push_str(&format!("/proc/{}/stat:{line}\n", spec.pid));
        }
    }
    text.push_str("\n@@T:end\n");
    text
}
