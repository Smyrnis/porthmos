use porthmos_core::edit::printable;

pub fn waiting_notice(name: &str) -> String {
    format!("Editing {}. Save the file in your editor, then press Done.", printable(name))
}

#[cfg(test)]
mod tests;
