use iced::Point;
use porthmos_core::listing::Row;
use porthmos_core::{Command, Entry, Location, VfsPath};

use crate::app::PanelSide;
use crate::native::NativeEvent;
use crate::panel::Panel;
use crate::transfers::transfer_command;

pub const THRESHOLD: f32 = 6.0;

#[derive(Debug, Clone, PartialEq)]
pub struct DragSource {
    pub side: PanelSide,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub side: PanelSide,
    pub location: Location,
    pub folder: VfsPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    ToItself,
    ToOwnChild,
    SameFolder,
    SamePanel,
    NoConnection,
    NothingDragged,
    DesktopToLocal,
}

impl Refusal {
    pub fn message(self) -> &'static str {
        match self {
            Refusal::ToItself => "A folder can't be dropped onto itself",
            Refusal::ToOwnChild => "A folder can't be dropped into one of its own subfolders",
            Refusal::SameFolder => "These files are already in that folder",
            Refusal::SamePanel => "Drop onto the other panel; use Rename to move within a panel",
            Refusal::NoConnection => "Connect to a server first to drop files onto the right panel",
            Refusal::NothingDragged => "There is nothing to drop",
            Refusal::DesktopToLocal => "Dropped files can only be uploaded: drop them on the right panel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEvent {
    None,
    Started,
    Updated,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Release {
    Nothing,
    Click,
    Drop { source: DragSource, at: Point },
    Abandoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Pressed,
    Internal,
    Native,
    Staging,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    Pressed {
        source: Option<DragSource>,
        origin: Option<Point>,
    },
    Active {
        source: DragSource,
        at: Point,
    },
    Native,
    Staging,
    Cancelled,
}

#[derive(Debug, Default)]
pub struct Drag {
    state: State,
}

impl Drag {
    pub fn press(&mut self, source: Option<DragSource>) {
        self.state = State::Pressed { source, origin: None };
    }

    pub fn moved(&mut self, to: Point) -> DragEvent {
        match &mut self.state {
            State::Pressed { origin, .. } if origin.is_none() => {
                *origin = Some(to);
                DragEvent::None
            }
            State::Pressed { source: Some(_), origin: Some(origin) } => {
                if (to.x - origin.x).hypot(to.y - origin.y) < THRESHOLD {
                    return DragEvent::None;
                }
                let State::Pressed { source: Some(source), .. } = std::mem::take(&mut self.state) else {
                    return DragEvent::None;
                };
                self.state = State::Active { source, at: to };
                DragEvent::Started
            }
            State::Active { at, .. } => {
                *at = to;
                DragEvent::Updated
            }
            _ => DragEvent::None,
        }
    }

    pub fn release(&mut self) -> Release {
        if matches!(self.state, State::Native) {
            return Release::Nothing;
        }
        match std::mem::take(&mut self.state) {
            State::Pressed { .. } => Release::Click,
            State::Active { source, at } => Release::Drop { source, at },
            State::Staging => Release::Abandoned,
            State::Idle | State::Cancelled | State::Native => Release::Nothing,
        }
    }

    pub fn cancel(&mut self) {
        if self.is_tracking() && !matches!(self.state, State::Native) {
            self.state = State::Cancelled;
        }
    }

    pub fn leave(&mut self) -> Option<DragSource> {
        let next = match &self.state {
            State::Active { source, .. } if source.side == PanelSide::Local => State::Native,
            State::Active { .. } => State::Staging,
            _ => return None,
        };
        match std::mem::replace(&mut self.state, next) {
            State::Active { source, .. } => Some(source),
            _ => None,
        }
    }

    pub fn staged(&mut self) -> bool {
        let staging = matches!(self.state, State::Staging);
        if staging {
            self.state = State::Native;
        }
        staging
    }

    pub fn abandon(&mut self) {
        if matches!(self.state, State::Native | State::Staging) {
            self.state = State::Idle;
        }
    }

    pub fn native_event(&mut self, event: NativeEvent) -> bool {
        if !matches!(self.state, State::Native) || event == NativeEvent::Started {
            return false;
        }
        self.state = State::Idle;
        true
    }

    pub fn phase(&self) -> Phase {
        match self.state {
            State::Idle | State::Cancelled => Phase::Idle,
            State::Pressed { .. } => Phase::Pressed,
            State::Active { .. } => Phase::Internal,
            State::Native => Phase::Native,
            State::Staging => Phase::Staging,
        }
    }

    pub fn is_tracking(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    #[cfg(test)]
    pub fn is_active(&self) -> bool {
        matches!(self.state, State::Active { .. })
    }

    pub fn pointer(&self) -> Option<Point> {
        match &self.state {
            State::Active { at, .. } => Some(*at),
            _ => None,
        }
    }

    pub fn source(&self) -> Option<&DragSource> {
        match &self.state {
            State::Active { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub fn dragged_entries(panel: &Panel, row: usize) -> Vec<Entry> {
    match panel.rows().get(row) {
        Some(Row::Entry(entry)) if panel.selected_paths().contains(&entry.path) => panel.selected(),
        Some(Row::Entry(entry)) => vec![entry.clone()],
        _ => Vec::new(),
    }
}

pub fn refusal(source: &DragSource, target: Option<&Target>) -> Option<Refusal> {
    if source.entries.is_empty() {
        return Some(Refusal::NothingDragged);
    }
    let Some(target) = target else {
        return Some(Refusal::NoConnection);
    };
    (target.side == source.side).then(|| refusal_within_a_panel(source, target))
}

pub fn decide(
    source: &DragSource, from: Location, target: Option<&Target>, move_requested: bool,
) -> Result<Command, Refusal> {
    if let Some(refusal) = refusal(source, target) {
        return Err(refusal);
    }
    let Some(target) = target else {
        return Err(Refusal::NoConnection);
    };
    Ok(transfer_command(move_requested, (from, source.entries.clone()), (target.location, target.folder.clone())))
}

pub fn drop_in_refusal(target: Option<&Target>) -> Option<Refusal> {
    match target {
        None => Some(Refusal::NoConnection),
        Some(target) if target.side == PanelSide::Local => Some(Refusal::DesktopToLocal),
        Some(_) => None,
    }
}

pub fn decide_drop_in(entries: &[Entry], target: Option<&Target>) -> Result<Command, Refusal> {
    if entries.is_empty() {
        return Err(Refusal::NothingDragged);
    }
    if let Some(refusal) = drop_in_refusal(target) {
        return Err(refusal);
    }
    let Some(target) = target else {
        return Err(Refusal::NoConnection);
    };
    Ok(transfer_command(false, (Location::Local, entries.to_vec()), (target.location, target.folder.clone())))
}

fn refusal_within_a_panel(source: &DragSource, target: &Target) -> Refusal {
    let folders = || source.entries.iter().filter(|entry| entry.is_dir);
    if folders().any(|entry| entry.path == target.folder) {
        Refusal::ToItself
    } else if folders().any(|entry| target.folder.starts_with(&entry.path)) {
        Refusal::ToOwnChild
    } else if source.entries.iter().all(|entry| entry.path.parent().as_ref() == Some(&target.folder)) {
        Refusal::SameFolder
    } else {
        Refusal::SamePanel
    }
}

#[cfg(test)]
mod tests;
