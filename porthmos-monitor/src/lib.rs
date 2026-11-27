use std::time::Duration;

mod arrange;
mod command;
mod model;
mod parse;
mod ports;
mod probe;
mod rate;
mod services;
#[cfg(test)]
mod testing;

pub use arrange::{Sort, arranged};
pub use command::{
    CommandError, Nice, SHELL_COMMAND, Signal, change_script, may_signal, renice_command, signal_command,
};
pub use model::{Cpu, Disk, Host, Interface, Load, Memory, Process, Section, Snapshot, UNKNOWN_UID};
pub use parse::sections::hides_or_reorders_text;
pub use parse::{Parsed, Sample, parse};
pub use ports::{
    MAX_SOCKETS, NetInterface, ParsedPorts, PortsSection, PortsSnapshot, Socket, SocketProtocol, SocketState,
    arranged_sockets, parse_ports, ports_command,
};
pub use probe::{Token, probe_command};
pub use services::{
    ActiveState, BootState, MAX_UNITS, ParsedServices, Privilege, ServiceVerb, ServicesSnapshot, StatusFilter,
    SudoProblem, Unit, UnitName, arranged_units, parse_services, service_command, services_command, status_command,
    sudo_problem, verb_allowed,
};

pub const POLL_TIMEOUT: Duration = Duration::from_secs(10);
pub const PROCESS_ACTION_MAX_AGE: Duration = Duration::from_secs(30);
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
pub const MAX_PROCESSES: usize = 200;
pub const MAX_LINE_BYTES: usize = 4096;
