use std::{
    io::{self, Stdout},
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type Backend = CrosstermBackend<Stdout>;

pub(crate) const KEYBOARD_FLAGS: KeyboardEnhancementFlags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;

static ENHANCEMENT_PUSHED: AtomicBool = AtomicBool::new(false);

pub(crate) fn enhancement_wanted(support: io::Result<bool>) -> bool {
    support.unwrap_or(false)
}

static ENHANCEMENT_SUPPORT: OnceLock<bool> = OnceLock::new();

pub(crate) fn cached_support(cell: &OnceLock<bool>, ask: impl FnOnce() -> io::Result<bool>) -> bool {
    *cell.get_or_init(|| enhancement_wanted(ask()))
}

fn push_keyboard_enhancement() {
    if cached_support(&ENHANCEMENT_SUPPORT, supports_keyboard_enhancement)
        && execute!(io::stdout(), PushKeyboardEnhancementFlags(KEYBOARD_FLAGS)).is_ok()
    {
        ENHANCEMENT_PUSHED.store(true, Ordering::Relaxed);
    }
}

fn pop_keyboard_enhancement() {
    if ENHANCEMENT_PUSHED.swap(false, Ordering::Relaxed) {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
}

pub fn init() -> Result<Terminal<Backend>> {
    enable_raw_mode()?;

    if let Err(err) = execute!(io::stdout(), EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(err.into());
    }

    match Terminal::new(CrosstermBackend::new(io::stdout())) {
        Ok(terminal) => {
            push_keyboard_enhancement();
            Ok(terminal)
        }
        Err(err) => {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            Err(err.into())
        }
    }
}

pub fn restore() -> Result<()> {
    pop_keyboard_enhancement();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, Show)?;
    Ok(())
}

#[cfg(test)]
mod tests;
