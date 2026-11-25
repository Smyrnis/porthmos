mod bookmarks;
mod connect;
mod count;
mod edit;
mod fs;
mod health;
mod history;
mod import;
mod last_used;
mod monitor;
mod moves;
mod pause;
mod preview;
mod profiles;
mod properties;
mod reconnect;
mod reopen;
mod reorder;
mod saved_transfers;
mod search;
mod secrets;
mod speed_limit;
mod sync;
mod terminal;
mod test_connection;
mod transfer;
mod tunnels;
mod ui_state;
mod update;

use std::fmt::Display;

pub(crate) use edit::{EditEvent, EditState};
pub(crate) use history::HistoryLog;
pub(crate) use import::ImportState;
pub(crate) use last_used::LastUsedLog;
pub(crate) use monitor::{MonitorRun, MonitorUpdate};
pub(crate) use moves::{MoveBatch, without_symlinks};
pub(crate) use preview::PreviewSlots;
#[cfg(test)]
pub(crate) use profiles::SecretOwner;
pub(crate) use profiles::{KeyringDone, KeyringState};
pub(crate) use properties::PropertiesState;
pub(crate) use reconnect::ReconnectState;
pub(crate) use reopen::ReopenState;
pub(crate) use saved_transfers::UnfinishedSaves;
pub(crate) use sync::SyncState;
pub(crate) use tunnels::TunnelRun;
pub(crate) use ui_state::StateGuard;
pub(crate) use update::UpdateEngine;

use super::Location;
use crate::{Message, failure_report};

fn failure_message(location: Location, context: impl AsRef<str>, err: &dyn Display) -> Message {
    match location {
        Location::Local => Message::new(err.to_string()),
        Location::Session(_) => failure_report(context, err),
    }
}
