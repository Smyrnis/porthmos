use std::path::{Path, PathBuf};

use anyhow::Result;
use iced::{Point, Size, window};
use porthmos_core::persist::{Loaded, read_or_set_aside, toml_problem, write_atomic};
use serde::{Deserialize, Serialize};

use crate::windows::WindowKind;

const FILE_NAME: &str = "gui-windows.toml";
const SMALLEST_SIDE: f32 = 100.0;
const LARGEST_SIDE: f32 = 16384.0;
const FARTHEST_COORDINATE: f32 = 16384.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Placement {
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowMemory {
    main: Placement,
    settings: Placement,
    connections: Placement,
    search: Placement,
}

pub struct MemoryLoad {
    pub memory: WindowMemory,
    pub warning: Option<String>,
}

pub fn file_in(state_dir: &Path) -> PathBuf {
    state_dir.join(FILE_NAME)
}

fn plausible_side(side: f32) -> bool {
    (SMALLEST_SIDE..=LARGEST_SIDE).contains(&side)
}

fn plausible_coordinate(coordinate: f32) -> bool {
    (-FARTHEST_COORDINATE..=FARTHEST_COORDINATE).contains(&coordinate)
}

impl Placement {
    fn checked(self) -> Placement {
        let size =
            self.width.zip(self.height).filter(|(width, height)| plausible_side(*width) && plausible_side(*height));
        let position = self.x.zip(self.y).filter(|(x, y)| plausible_coordinate(*x) && plausible_coordinate(*y));
        Placement {
            width: size.map(|(width, _)| width),
            height: size.map(|(_, height)| height),
            x: position.map(|(x, _)| x),
            y: position.map(|(_, y)| y),
        }
    }
}

impl WindowMemory {
    fn placement(&self, kind: WindowKind) -> &Placement {
        match kind {
            WindowKind::Main => &self.main,
            WindowKind::Settings => &self.settings,
            WindowKind::Connections => &self.connections,
            WindowKind::Search => &self.search,
        }
    }

    fn placement_mut(&mut self, kind: WindowKind) -> &mut Placement {
        match kind {
            WindowKind::Main => &mut self.main,
            WindowKind::Settings => &mut self.settings,
            WindowKind::Connections => &mut self.connections,
            WindowKind::Search => &mut self.search,
        }
    }

    fn checked(self) -> WindowMemory {
        WindowMemory {
            main: self.main.checked(),
            settings: self.settings.checked(),
            connections: self.connections.checked(),
            search: self.search.checked(),
        }
    }

    pub fn load(file: &Path) -> MemoryLoad {
        match read_or_set_aside(file, "window geometry", |text| {
            toml::from_str::<WindowMemory>(text).map_err(|err| toml_problem(text, &err))
        }) {
            Loaded::Missing => MemoryLoad { memory: WindowMemory::default(), warning: None },
            Loaded::Ready(memory) => MemoryLoad { memory: memory.checked(), warning: None },
            Loaded::SetAside { warning, .. } => MemoryLoad { memory: WindowMemory::default(), warning: Some(warning) },
        }
    }

    pub fn save(&self, file: &Path) -> Result<()> {
        write_atomic(file, toml::to_string(self)?.as_bytes(), 0o600)
    }

    pub fn resized(&mut self, kind: WindowKind, size: Size) {
        if plausible_side(size.width) && plausible_side(size.height) {
            let placement = self.placement_mut(kind);
            placement.width = Some(size.width);
            placement.height = Some(size.height);
        }
    }

    pub fn moved(&mut self, kind: WindowKind, position: Point) {
        if plausible_coordinate(position.x) && plausible_coordinate(position.y) {
            let placement = self.placement_mut(kind);
            placement.x = Some(position.x);
            placement.y = Some(position.y);
        }
    }

    pub fn applied_to(&self, kind: WindowKind, mut settings: window::Settings) -> window::Settings {
        let placement = self.placement(kind);
        if let Some((width, height)) = placement.width.zip(placement.height) {
            let floor = settings.min_size.unwrap_or(Size::ZERO);
            settings.size = Size::new(width.max(floor.width), height.max(floor.height));
        }
        if let Some((x, y)) = placement.x.zip(placement.y) {
            settings.position = window::Position::Specific(Point::new(x, y));
        }
        settings
    }
}

#[cfg(test)]
mod tests;
