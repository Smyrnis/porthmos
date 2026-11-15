use porthmos_vfs::{FileKind, FileSystem, ProtocolError, VfsPath};

pub(crate) struct PermissionTree {
    pub paths: Vec<VfsPath>,
    pub first_unreadable: Option<(VfsPath, ProtocolError)>,
}

pub(crate) async fn expand(fs: &dyn FileSystem, roots: Vec<VfsPath>) -> PermissionTree {
    let mut paths = Vec::new();
    let mut first_unreadable = None;
    let mut pending: Vec<VfsPath> = Vec::new();
    for root in roots {
        paths.push(root.clone());
        if fs.stat(&root).await.is_ok_and(|metadata| metadata.kind == FileKind::Dir) && !is_link(fs, &root).await {
            pending.push(root);
        }
    }
    while let Some(folder) = pending.pop() {
        match fs.read_dir(&folder).await {
            Ok(items) => {
                for item in items {
                    match item.metadata.kind {
                        FileKind::Symlink => {}
                        FileKind::File => paths.push(folder.join(&item.name)),
                        FileKind::Dir => {
                            let child = folder.join(&item.name);
                            paths.push(child.clone());
                            pending.push(child);
                        }
                    }
                }
            }
            Err(err) if first_unreadable.is_none() => first_unreadable = Some((folder, err)),
            Err(_) => {}
        }
    }
    PermissionTree { paths, first_unreadable }
}

async fn is_link(fs: &dyn FileSystem, path: &VfsPath) -> bool {
    let Some(parent) = path.parent().filter(|_| path.file_name().is_some()) else {
        return false;
    };
    fs.read_dir(&parent)
        .await
        .ok()
        .and_then(|items| items.into_iter().find(|item| item.path == *path))
        .is_some_and(|item| item.metadata.kind == FileKind::Symlink)
}
