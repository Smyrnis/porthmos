use std::{ffi::CString, io, mem::MaybeUninit, os::unix::ffi::OsStrExt, path::Path};

fn available_bytes(blocks: u64, block_size: u64) -> u64 {
    blocks.saturating_mul(block_size)
}

#[allow(clippy::unnecessary_cast)]
pub(crate) fn free_space(path: &Path) -> io::Result<u64> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the path contains a NUL byte"))?;
    let mut stats = MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stats = unsafe { stats.assume_init() };
    Ok(available_bytes(stats.f_bavail as u64, stats.f_frsize as u64))
}

#[cfg(test)]
mod tests;
