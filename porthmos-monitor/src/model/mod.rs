pub const UNKNOWN_UID: u32 = u32::MAX;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Host {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cpu {
    pub cores: Option<u32>,
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Load {
    pub one: f32,
    pub five: f32,
    pub fifteen: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    pub mount: String,
    pub device: String,
    pub total: u64,
    pub used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub rx_per_second: Option<u64>,
    pub tx_per_second: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub pid: u32,
    pub uid: u32,
    pub user: Option<String>,
    pub name: String,
    pub command: String,
    pub cpu_percent: Option<f32>,
    pub memory_percent: f32,
    pub nice: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Host,
    Cpu,
    Load,
    Memory,
    Disks,
    Network,
    Processes,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub host: Host,
    pub cpu: Cpu,
    pub load: Option<Load>,
    pub memory: Option<Memory>,
    pub disks: Vec<Disk>,
    pub interfaces: Vec<Interface>,
    pub processes: Vec<Process>,
    pub uid: u32,
    pub problems: Vec<Section>,
}
