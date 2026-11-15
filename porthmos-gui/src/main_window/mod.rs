use iced::{Size, window};

pub const INITIAL_SIZE: Size = Size::new(1100.0, 700.0);
const MINIMUM_SIZE: Size = Size::new(480.0, 320.0);

pub fn settings() -> window::Settings {
    window::Settings {
        size: INITIAL_SIZE,
        min_size: Some(MINIMUM_SIZE),
        exit_on_close_request: false,
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: porthmos_core::Product::Gui.program().to_string(),
            ..Default::default()
        },
        ..window::Settings::default()
    }
}

#[cfg(test)]
mod tests;
