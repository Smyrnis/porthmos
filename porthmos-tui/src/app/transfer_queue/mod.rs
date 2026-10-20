use super::*;

const RATE_TICK: Duration = Duration::from_secs(1);

impl App {
    pub(super) fn open_transfers_screen(&mut self) {
        self.screen = Screen::Transfers;
        self.transfers_cursor = 0;
    }

    pub(super) fn transfer_rows(&self) -> &[QueueRow] {
        &self.transfers.rows
    }

    pub(super) fn apply_transfer_snapshot(&mut self, snapshot: TransferSnapshot) {
        self.transfers = snapshot;
        self.tick_rates(Instant::now());
        self.track_failure_times();
        if let Some(kind) = self.reselect_row.take() {
            self.transfers_cursor = self
                .transfer_rows()
                .iter()
                .position(|row| row.kind == kind)
                .unwrap_or_else(|| self.clamped_transfers_cursor());
        }
    }

    pub(super) fn tick_rates(&mut self, now: Instant) {
        self.rates.observe(now, &self.transfers.rows);
        self.rate_observed_at = now;
    }

    pub(super) fn next_rate_wake(&self) -> Option<Instant> {
        let any_running = self.transfers.rows.iter().any(|row| row.state == RowState::Running);
        any_running.then(|| self.rate_observed_at + RATE_TICK)
    }

    fn track_failure_times(&mut self) {
        let failing: HashSet<RowKind> = self
            .transfers
            .rows
            .iter()
            .filter(|row| matches!(row.state, RowState::Failed | RowState::PartlyFailed(_)))
            .map(|row| row.kind)
            .collect();
        self.failed_at.retain(|kind, _| failing.contains(kind));
        let now = self.clock.now();
        for kind in failing {
            self.failed_at.entry(kind).or_insert(now);
        }
    }

    pub(super) fn failed_time_of(&self, kind: RowKind) -> Option<&DateTime<Local>> {
        self.failed_at.get(&kind)
    }

    pub(super) fn route_of(&self, row: &QueueRow) -> String {
        let Some(session) = row.session_id else { return String::new() };
        let name = self.session_names.get(&session).map_or("remote", String::as_str);
        match row.direction {
            Direction::Upload => format!("local \u{2192} {name}"),
            Direction::Download => format!("{name} \u{2192} local"),
        }
    }

    pub(super) fn apply_transfers_action(&mut self, action: Action) {
        match action {
            Action::Up => self.transfers_cursor = self.clamped_transfers_cursor().saturating_sub(1),
            Action::Down => {
                let last = self.transfer_rows().len().saturating_sub(1);
                self.transfers_cursor = (self.clamped_transfers_cursor() + 1).min(last);
            }
            Action::Open => {
                if let Some(kind) = self.selected_row_kind() {
                    self.core.send(Command::RetryRow { kind });
                }
            }
            Action::Refresh => {
                if self.transfer_rows().is_empty() {
                    return;
                }
                self.reselect_row = self.selected_row_kind();
                self.core.send(Command::ClearFinished);
            }
            _ => {}
        }
    }

    pub(super) fn cancel_selected_row(&mut self) {
        if let Some(kind) = self.selected_row_kind() {
            self.core.send(Command::CancelRow { kind });
        }
    }

    fn selected_row_kind(&self) -> Option<RowKind> {
        self.transfer_rows().get(self.clamped_transfers_cursor()).map(|row| row.kind)
    }

    fn clamped_transfers_cursor(&self) -> usize {
        self.transfers_cursor.min(self.transfer_rows().len().saturating_sub(1))
    }
}

#[cfg(test)]
mod tests;
