use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub(crate) const PROBE_INTERVAL: Duration = Duration::from_secs(10);
const CANCEL_POLL: Duration = Duration::from_millis(250);

pub(crate) async fn wait_unless_cancelled(total: Duration, cancel: &AtomicBool) -> bool {
    let mut left = total;
    while !left.is_zero() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let slice = left.min(CANCEL_POLL);
        tokio::time::sleep(slice).await;
        left -= slice;
    }
    !cancel.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests;
