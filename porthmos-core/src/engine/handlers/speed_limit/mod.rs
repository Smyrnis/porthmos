use super::super::Engine;

impl Engine {
    pub(crate) fn set_speed_limit(&mut self, bytes_per_second: Option<u64>) {
        let limit = bytes_per_second.filter(|rate| *rate > 0);
        self.speed_limit.set(limit);
        self.ui_state.speed_limit = limit;
        self.remember_ui_state("speed limit");
        self.publish_transfers();
    }
}

#[cfg(test)]
mod tests;
