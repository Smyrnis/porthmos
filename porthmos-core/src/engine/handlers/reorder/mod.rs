use super::super::Engine;
use crate::transfer::rows::RowKind;

impl Engine {
    pub(crate) fn reorder_row(&mut self, kind: RowKind, earlier: bool) {
        if self.transfers.move_row(kind, earlier) {
            self.publish_transfers();
        }
    }
}

#[cfg(test)]
mod tests;
