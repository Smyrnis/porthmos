#[cfg(windows)]
pub fn attach_parent() {
    use std::fs::OpenOptions;
    use std::os::windows::io::IntoRawHandle;

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
    };

    let attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0;
    if !attached {
        return;
    }
    for stream in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        let current = unsafe { GetStdHandle(stream) };
        if !current.is_null() && current != INVALID_HANDLE_VALUE {
            continue;
        }
        if let Ok(console) = OpenOptions::new().write(true).open("CONOUT$") {
            unsafe { SetStdHandle(stream, console.into_raw_handle()) };
        }
    }
}

#[cfg(not(windows))]
pub fn attach_parent() {}
