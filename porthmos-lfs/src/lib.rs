mod attributes;
mod in_use;
mod os_path;
#[cfg(unix)]
mod ownership;
mod roots;
mod space;

use in_use::explain;
pub use os_path::{checked_os_path, from_os_path, to_os_path};

use std::{
    fs, io,
    io::SeekFrom,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use porthmos_vfs::{
    DirItem, Entry, FileKind, FileSystem, Metadata, NameRules, ProtocolError, Reader, VfsPath, Writer, async_trait,
};
use tokio::io::AsyncSeekExt;

pub struct LocalFs {
    home: PathBuf,
    name_rules: NameRules,
}

impl LocalFs {
    pub fn new(home: PathBuf) -> Self {
        Self { home, name_rules: platform_name_rules() }
    }

    pub fn with_name_rules(mut self, rules: NameRules) -> Self {
        self.name_rules = rules;
        self
    }
}

#[cfg(unix)]
fn platform_name_rules() -> NameRules {
    NameRules::Permissive
}

#[cfg(windows)]
fn platform_name_rules() -> NameRules {
    NameRules::Windows
}

fn metadata_from(metadata: &fs::Metadata) -> Metadata {
    let kind = if metadata.file_type().is_symlink() {
        FileKind::Symlink
    } else if metadata.is_dir() {
        FileKind::Dir
    } else {
        FileKind::File
    };
    Metadata {
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_secs()),
        kind,
        permissions: attributes::permissions_of(metadata),
    }
}

#[cfg(unix)]
fn open_for_times(path: &Path) -> io::Result<fs::File> {
    fs::File::open(path)
}

#[cfg(windows)]
fn open_for_times(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;

    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_WRITE_ATTRIBUTES};

    fs::OpenOptions::new().access_mode(FILE_WRITE_ATTRIBUTES).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(path)
}

fn mapped(path: &VfsPath) -> Result<PathBuf, ProtocolError> {
    checked_os_path(path).map_err(ProtocolError::from)
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, ProtocolError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|join_error| ProtocolError::from(anyhow::Error::from(join_error)))?
        .map_err(explain)
}

#[async_trait]
impl FileSystem for LocalFs {
    fn name_rules(&self) -> NameRules {
        self.name_rules
    }

    async fn set_modified(&self, path: &VfsPath, seconds: u64) -> Result<(), ProtocolError> {
        let path = mapped(path)?;
        blocking(move || {
            let moment = UNIX_EPOCH.checked_add(Duration::from_secs(seconds)).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("the modification time {seconds} is out of range"))
            })?;
            open_for_times(&path)?.set_modified(moment)
        })
        .await
    }

    fn can_set_modified(&self) -> bool {
        true
    }

    async fn list(&self, dir: &VfsPath) -> Result<Vec<Entry>, ProtocolError> {
        #[cfg(windows)]
        if roots::lists_drives(dir) {
            return Ok(roots::entries_for(&roots::present_drives()));
        }
        let dir = mapped(dir)?;
        blocking(move || list(&dir)).await
    }

    async fn free_space(&self, dir: &VfsPath) -> Result<Option<u64>, ProtocolError> {
        if roots::lists_drives(dir) {
            return Ok(None);
        }
        let dir = mapped(dir)?;
        blocking(move || space::free_space(&dir).map(Some)).await
    }

    async fn read_dir(&self, dir: &VfsPath) -> Result<Vec<DirItem>, ProtocolError> {
        #[cfg(windows)]
        if roots::lists_drives(dir) {
            return Ok(roots::items_for(&roots::present_drives()));
        }
        let dir = mapped(dir)?;
        blocking(move || {
            let mut items = Vec::new();
            for dir_entry in fs::read_dir(&dir)? {
                let dir_entry = dir_entry?;
                let metadata = fs::symlink_metadata(dir_entry.path())?;
                items.push(DirItem {
                    name: dir_entry.file_name().to_string_lossy().into_owned(),
                    path: from_os_path(&dir_entry.path()),
                    metadata: metadata_from(&metadata),
                });
            }
            Ok(items)
        })
        .await
    }

    async fn stat(&self, path: &VfsPath) -> Result<Metadata, ProtocolError> {
        #[cfg(windows)]
        if roots::lists_drives(path) {
            return Ok(roots::root_metadata());
        }
        Ok(metadata_from(&tokio::fs::metadata(mapped(path)?).await.map_err(explain)?))
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        tokio::fs::create_dir(mapped(path)?).await.map_err(explain)
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), ProtocolError> {
        tokio::fs::rename(mapped(from)?, mapped(to)?).await.map_err(explain)
    }

    async fn remove_file(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        tokio::fs::remove_file(mapped(path)?).await.map_err(explain)
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let path = mapped(path)?;
        blocking(move || delete(&path)).await
    }

    async fn remove_dir(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let path = mapped(path)?;
        blocking(move || fs::remove_dir(&path)).await
    }

    async fn home(&self) -> Result<VfsPath, ProtocolError> {
        Ok(from_os_path(&self.home))
    }

    async fn open_read(&self, path: &VfsPath, offset: u64) -> Result<Reader, ProtocolError> {
        let mut file = tokio::fs::File::open(mapped(path)?).await.map_err(explain)?;
        file.seek(SeekFrom::Start(offset)).await.map_err(explain)?;
        Ok(Box::new(file))
    }

    async fn open_write(&self, path: &VfsPath, offset: u64) -> Result<Writer, ProtocolError> {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(mapped(path)?)
            .await
            .map_err(explain)?;
        file.set_len(offset).await.map_err(explain)?;
        file.seek(SeekFrom::Start(offset)).await.map_err(explain)?;
        Ok(Writer { stream: Box::new(file), offset })
    }
}

pub fn list(path: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut resolver = attributes::AttributeResolver::new();

    for dir_entry in fs::read_dir(path)? {
        let dir_entry = dir_entry?;
        let metadata = dir_entry.metadata()?;
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        let link_target = metadata.file_type().is_symlink().then(|| {
            fs::read_link(dir_entry.path()).map(|target| target.to_string_lossy().into_owned()).unwrap_or_default()
        });

        let attributes = resolver.of(&metadata);

        entries.push(Entry {
            name,
            path: from_os_path(&dir_entry.path()),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            permissions: attributes.permissions,
            modified: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_secs()),
            owner: attributes.owner,
            group: attributes.group,
            hidden: attributes.hidden,
            link_target,
        });
    }

    Ok(entries)
}

pub fn create_directory(path: &Path) -> io::Result<()> {
    fs::create_dir(path)?;
    Ok(())
}

pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)?;
    Ok(())
}

pub fn delete(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;

    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
