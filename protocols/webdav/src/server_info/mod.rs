use porthmos_vfs::{SERVER_INFO_CELLS, clean_server_text};
use reqwest::header::{HeaderMap, HeaderName, SERVER};

const DAV: HeaderName = HeaderName::from_static("dav");

fn text_of(headers: &HeaderMap, name: &HeaderName) -> Option<String> {
    let value = headers.get(name)?;
    clean_server_text(&String::from_utf8_lossy(value.as_bytes()), SERVER_INFO_CELLS)
}

pub(crate) fn server_info(headers: &HeaderMap) -> Option<String> {
    text_of(headers, &SERVER).or_else(|| {
        let classes = text_of(headers, &DAV)?;
        clean_server_text(&format!("DAV {classes}"), SERVER_INFO_CELLS)
    })
}

#[cfg(test)]
mod tests;
