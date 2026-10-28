use std::fs;

#[cfg(unix)]
use crate::ownership::{NameCache, group_name, user_name};

pub(crate) struct Attributes {
    pub(crate) permissions: Option<u32>,
    pub(crate) owner: Option<String>,
    pub(crate) group: Option<String>,
    pub(crate) hidden: bool,
}

#[cfg(any(windows, test))]
const HIDDEN_ATTRIBUTE: u32 = 0x2;
#[cfg(any(windows, test))]
const SYSTEM_ATTRIBUTE: u32 = 0x4;

#[cfg(any(windows, test))]
pub(crate) fn is_hidden_attribute(attributes: u32) -> bool {
    attributes & (HIDDEN_ATTRIBUTE | SYSTEM_ATTRIBUTE) != 0
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
            hidden: false,
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
        use std::os::windows::fs::MetadataExt;

        Attributes {
            permissions: permissions_of(metadata),
            owner: None,
            group: None,
            hidden: is_hidden_attribute(metadata.file_attributes()),
        }
    }
}

#[cfg(test)]
mod tests;
