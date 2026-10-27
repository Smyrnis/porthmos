use std::fs;

#[cfg(unix)]
use crate::ownership::{NameCache, group_name, user_name};

pub(crate) struct Attributes {
    pub(crate) permissions: Option<u32>,
    pub(crate) owner: Option<String>,
    pub(crate) group: Option<String>,
}

pub(crate) fn permissions_of(metadata: &fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        Some(metadata.permissions().mode())
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        None
    }
}

#[cfg(unix)]
pub(crate) struct AttributeResolver {
    users: NameCache,
    groups: NameCache,
}

#[cfg(unix)]
impl AttributeResolver {
    pub(crate) fn new() -> Self {
        Self { users: NameCache::new(user_name), groups: NameCache::new(group_name) }
    }

    pub(crate) fn of(&mut self, metadata: &fs::Metadata) -> Attributes {
        use std::os::unix::fs::MetadataExt;

        Attributes {
            permissions: permissions_of(metadata),
            owner: Some(self.users.name(metadata.uid())),
            group: Some(self.groups.name(metadata.gid())),
        }
    }
}

#[cfg(windows)]
pub(crate) struct AttributeResolver;

#[cfg(windows)]
impl AttributeResolver {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn of(&mut self, metadata: &fs::Metadata) -> Attributes {
        Attributes { permissions: permissions_of(metadata), owner: None, group: None }
    }
}
