use std::{
    fs::{self, DirBuilder, OpenOptions},
    io,
    path::Path,
};

const PRIVATE_DIR_MODE: u32 = 0o700;

pub(crate) fn private_file_options(mode: u32) -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(mode);
    }
    #[cfg(windows)]
    let _ = mode;
    options
}

pub(crate) fn restrict_to_owner(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(mode))
    }
    #[cfg(windows)]
    {
        let _ = mode;
        fs::metadata(path).map(drop)
    }
}

fn private_dir_builder(recursive: bool) -> DirBuilder {
    let mut builder = DirBuilder::new();
    builder.recursive(recursive);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        builder.mode(PRIVATE_DIR_MODE);
    }
    #[cfg(windows)]
    let _ = PRIVATE_DIR_MODE;
    builder
}

pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    if let Some(parent) = dir.parent() {
        private_dir_builder(true).create(parent)?;
    }
    private_dir_builder(false).create(dir)
}
