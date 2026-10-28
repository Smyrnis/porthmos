mod bookmarks;
mod connect;
mod edit;
mod fs;
mod health;
mod history;
mod last_used;
mod moves;
mod pause;
mod preview;
mod profiles;
mod reorder;
mod search;
mod secrets;
mod speed_limit;
mod sync;
mod test_connection;
mod transfer;
mod ui_state;
mod update;

use std::fmt::Display;

pub(crate) use edit::{EditEvent, EditState};
pub(crate) use history::HistoryLog;
pub(crate) use last_used::LastUsedLog;
pub(crate) use moves::{MoveBatch, without_symlinks};
#[cfg(test)]
pub(crate) use profiles::SecretOwner;
pub(crate) use profiles::{KeyringDone, KeyringJob};
pub(crate) use sync::SyncState;
pub(crate) use update::UpdateEngine;

use super::Location;
use crate::user_message;

fn failure_message(location: Location, context: impl AsRef<str>, err: &dyn Display) -> String {
    match location {
        Location::Local => err.to_string(),
        Location::Session(_) => user_message(context, err),
    }
}
