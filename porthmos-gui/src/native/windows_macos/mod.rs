use std::path::PathBuf;

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};

use super::{EventLog, Native, NativeError, NativeEvent};

const ICON: &[u8] = include_bytes!("drag-icon.png");

struct Handle(RawWindowHandle);

unsafe impl Send for Handle {}

impl HasWindowHandle for Handle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe { WindowHandle::borrow_raw(self.0) })
    }
}

pub struct OsNative {
    handle: Handle,
    events: EventLog,
}

impl std::fmt::Debug for OsNative {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OsNative")
    }
}

impl OsNative {
    pub fn attach(window: &dyn iced::window::Window) -> Option<OsNative> {
        let raw = window.window_handle().ok()?.as_raw();
        Some(OsNative { handle: Handle(raw), events: EventLog::default() })
    }
}

impl Native for OsNative {
    fn start_drag(&mut self, paths: Vec<PathBuf>) -> Result<(), NativeError> {
        let events = self.events.clone();
        self.events.push(NativeEvent::Started);
        drag::start_drag(
            &self.handle,
            drag::DragItem::Files(paths),
            drag::Image::Raw(ICON.to_vec()),
            move |result, _position| {
                events.push(match result {
                    drag::DragResult::Dropped => NativeEvent::Finished,
                    drag::DragResult::Cancel => NativeEvent::Cancelled,
                })
            },
            drag::Options::default(),
        )
        .map_err(|error| NativeError(error.to_string()))
    }

    fn cancel(&mut self) {}

    fn poll(&mut self) -> Vec<NativeEvent> {
        self.events.take()
    }
}
