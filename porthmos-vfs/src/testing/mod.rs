mod fake_fs;
mod fake_protocol;

pub use fake_fs::FakeFs;
pub use fake_protocol::FakeProtocol;

#[cfg(unix)]
pub fn read_only_folders_block_writes() -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Ok(dir) = tempfile::tempdir() else { return false };
    let permissions = std::fs::Permissions::from_mode(0o555);
    if std::fs::set_permissions(dir.path(), permissions).is_err() {
        return false;
    }
    let blocked = std::fs::File::create(dir.path().join("probe")).is_err();
    let _ = std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700));
    blocked
}

#[cfg(test)]
mod tests;
