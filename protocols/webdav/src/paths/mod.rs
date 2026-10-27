use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use porthmos_vfs::VfsPath;

const PARENT: &str = "..";
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Locator {
    origin: String,
    root: Vec<String>,
}

impl Locator {
    pub(crate) fn new(origin: &str, root: &str) -> Self {
        let root = root
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| percent_decode_str(segment).decode_utf8_lossy().into_owned())
            .collect();
        Self { origin: origin.to_string(), root }
    }

    pub(crate) fn href(&self, path: &VfsPath, collection: bool) -> String {
        let encoded: Vec<String> = self
            .root
            .iter()
            .cloned()
            .chain(segments(path))
            .map(|segment| utf8_percent_encode(&segment, SEGMENT).to_string())
            .collect();
        let mut href = format!("/{}", encoded.join("/"));
        if collection && !href.ends_with('/') {
            href.push('/');
        }
        href
    }

    pub(crate) fn root(&self) -> String {
        if self.root.is_empty() { "/".to_string() } else { format!("/{}/", self.root.join("/")) }
    }

    pub(crate) fn url(&self, href: &str) -> String {
        format!("{}{href}", self.origin)
    }

    pub(crate) fn path_of_href(&self, href: &str) -> Option<VfsPath> {
        let absolute = match href.find("://") {
            Some(scheme_end) => {
                let rest = &href[scheme_end + 3..];
                rest.find('/').map_or("/", |path_start| &rest[path_start..])
            }
            None => href,
        };
        let without_query = absolute.split(['?', '#']).next().unwrap_or("/");
        let decoded: Vec<String> = without_query
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| percent_decode_str(segment).decode_utf8_lossy().into_owned())
            .collect();
        let inside = decoded.strip_prefix(self.root.as_slice())?;
        Some(to_path(inside))
    }
}

pub(crate) fn normalize(path: &VfsPath) -> VfsPath {
    to_path(&segments(path))
}

fn segments(path: &VfsPath) -> Vec<String> {
    let mut segments = Vec::new();
    for segment in path.segments() {
        if segment == PARENT {
            segments.pop();
        } else {
            segments.push(segment.to_string_lossy().into_owned());
        }
    }
    segments
}

fn to_path(segments: &[String]) -> VfsPath {
    VfsPath::from_segments(true, segments)
}

#[cfg(test)]
mod tests;
