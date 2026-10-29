use porthmos_core::VfsPath;

use crate::app::PanelSide;

const LIMIT: usize = 100;

#[derive(Debug, Clone, Default)]
pub struct History {
    back: Vec<VfsPath>,
    forward: Vec<VfsPath>,
}

impl History {
    pub fn new() -> History {
        History::default()
    }

    pub fn visit(&mut self, from: VfsPath) {
        self.back.push(from);
        if self.back.len() > LIMIT {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    pub fn back(&mut self, current: VfsPath) -> Option<VfsPath> {
        let target = self.back.pop()?;
        self.forward.push(current);
        Some(target)
    }

    pub fn forward(&mut self, current: VfsPath) -> Option<VfsPath> {
        let target = self.forward.pop()?;
        self.back.push(current);
        Some(target)
    }

    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEdit {
    pub side: PanelSide,
    pub text: String,
}

#[cfg(test)]
mod tests;
