use std::fs;

use crate::Paths;

pub(crate) fn remove(paths: &Paths) {
    for name in ["update.toml", "update.toml.broken"] {
        let _ = fs::remove_file(paths.state_dir.join(name));
    }
    porthmos_update::remove_legacy_layout(&paths.update_dir());
}

#[cfg(test)]
mod tests;
