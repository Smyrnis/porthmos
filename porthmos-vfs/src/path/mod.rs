use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    fmt,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, ser};

const SEPARATOR: char = '/';
const CURRENT: &str = ".";
const PARENT: &str = "..";

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct VfsPath {
    absolute: bool,
    segments: Vec<OsString>,
}

impl VfsPath {
    pub fn root() -> Self {
        Self { absolute: true, segments: Vec::new() }
    }

    pub fn new(text: &str) -> Self {
        Self::from_segments(text.starts_with(SEPARATOR), text.split(SEPARATOR))
    }

    pub fn from_segments<S: Into<OsString>>(absolute: bool, segments: impl IntoIterator<Item = S>) -> Self {
        let segments =
            segments.into_iter().map(Into::into).filter(|segment: &OsString| !segment.is_empty() && segment != CURRENT);
        Self { absolute, segments: segments.collect() }
    }

    pub fn is_absolute(&self) -> bool {
        self.absolute
    }

    pub fn is_root(&self) -> bool {
        self.absolute && self.segments.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        !self.absolute && self.segments.is_empty()
    }

    pub fn join(&self, relative: impl Into<VfsPath>) -> VfsPath {
        let relative = relative.into();
        if relative.absolute {
            return relative;
        }
        let mut segments = self.segments.clone();
        segments.extend(relative.segments);
        Self { absolute: self.absolute, segments }
    }

    pub fn parent(&self) -> Option<VfsPath> {
        if self.segments.is_empty() {
            return None;
        }
        let mut segments = self.segments.clone();
        segments.pop();
        Some(Self { absolute: self.absolute, segments })
    }

    pub fn file_name(&self) -> Option<&OsStr> {
        self.segments.last().map(OsString::as_os_str).filter(|name| *name != OsStr::new(PARENT))
    }

    pub fn segments(&self) -> impl Iterator<Item = &OsStr> {
        self.segments.iter().map(OsString::as_os_str)
    }

    pub fn ancestors(&self) -> impl Iterator<Item = VfsPath> {
        std::iter::successors(Some(self.clone()), VfsPath::parent)
    }

    pub fn starts_with(&self, base: &VfsPath) -> bool {
        self.absolute == base.absolute && self.segments.starts_with(&base.segments)
    }

    pub fn strip_prefix(&self, base: &VfsPath) -> Option<VfsPath> {
        self.starts_with(base)
            .then(|| Self { absolute: false, segments: self.segments[base.segments.len()..].to_vec() })
    }

    pub fn with_file_name(&self, name: &str) -> VfsPath {
        let mut segments = self.segments.clone();
        segments.pop();
        segments.extend(name.split(SEPARATOR).map(OsString::from));
        Self::from_segments(self.absolute, segments)
    }

    pub fn with_suffix(&self, suffix: &str) -> VfsPath {
        let mut segments = self.segments.clone();
        match segments.last_mut() {
            Some(last) => last.push(suffix),
            None => segments.push(OsString::from(suffix)),
        }
        Self { absolute: self.absolute, segments }
    }

    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        let mut text = String::new();
        if self.absolute {
            text.push(SEPARATOR);
        }
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 {
                text.push(SEPARATOR);
            }
            text.push_str(&segment.to_string_lossy());
        }
        Cow::Owned(text)
    }
}

impl From<&str> for VfsPath {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<&String> for VfsPath {
    fn from(text: &String) -> Self {
        Self::new(text)
    }
}

impl From<&VfsPath> for VfsPath {
    fn from(path: &VfsPath) -> Self {
        path.clone()
    }
}

impl From<String> for VfsPath {
    fn from(text: String) -> Self {
        Self::new(&text)
    }
}

impl fmt::Display for VfsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

impl fmt::Debug for VfsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VfsPath({:?})", self.to_string_lossy())
    }
}

impl Serialize for VfsPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.segments.iter().any(|segment| segment.to_str().is_none()) {
            return Err(ser::Error::custom("the path is not valid UTF-8"));
        }
        serializer.serialize_str(&self.to_string_lossy())
    }
}

impl<'de> Deserialize<'de> for VfsPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(test)]
mod tests;
