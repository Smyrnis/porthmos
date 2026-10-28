use std::{io, path::Path};

#[cfg(unix)]
fn available_bytes(blocks: u64, block_size: u64) -> u64 {
    blocks.saturating_mul(block_size)
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
pub(crate) fn free_space(path: &Path) -> io::Result<u64> {
    use std::{ffi::CString, mem::MaybeUninit, os::unix::ffi::OsStrExt};

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the path contains a NUL byte"))?;
    let mut stats = MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stats = unsafe { stats.assume_init() };
    Ok(available_bytes(stats.f_bavail as u64, stats.f_frsize as u64))
}

#[cfg(windows)]
pub(crate) fn free_space(path: &Path) -> io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "the path is empty or contains a NUL character");
    if path.as_os_str().is_empty() || path.as_os_str().encode_wide().any(|unit| unit == 0) {
        return Err(invalid());
    }
    let directory = if std::fs::metadata(path)?.is_dir() { path } else { path.parent().ok_or_else(invalid)? };
    let wide: Vec<u16> = directory.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut available = 0u64;
    let succeeded =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, std::ptr::null_mut(), std::ptr::null_mut()) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(available)
}

#[cfg(test)]
mod tests;
