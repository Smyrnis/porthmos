use std::error::Error as _;

use anyhow::anyhow;
use porthmos_vfs::{ErrorKind, ProtocolError, VfsPath};
use reqwest::{
    StatusCode,
    header::{HeaderMap, LOCATION},
};

use crate::idle::NO_RESPONSE;

pub(crate) fn status_error(status: StatusCode, headers: &HeaderMap, path: &VfsPath) -> ProtocolError {
    if status.is_redirection() {
        return redirect_error(headers);
    }
    match status.as_u16() {
        401 => ProtocolError::new(ErrorKind::Auth, anyhow!("authentication expired")),
        403 => ProtocolError::new(ErrorKind::PermissionDenied, anyhow!("{path}: permission denied")),
        404 | 409 => ProtocolError::new(ErrorKind::NotFound, anyhow!("{path} not found")),
        423 => ProtocolError::new(ErrorKind::Other, anyhow!("{path} is locked")),
        507 => ProtocolError::new(ErrorKind::Other, anyhow!("insufficient storage on the server")),
        _ => ProtocolError::new(ErrorKind::Other, anyhow!("{status}")),
    }
}

pub(crate) fn redirect_error(headers: &HeaderMap) -> ProtocolError {
    let location = headers.get(LOCATION).and_then(|value| value.to_str().ok()).unwrap_or("another address");
    ProtocolError::new(
        ErrorKind::Connect,
        anyhow!("the server redirected to {location} \u{2014} check Security and Root path"),
    )
}

pub(crate) enum Failure {
    Request(reqwest::Error),
    TimedOut,
}

impl From<reqwest::Error> for Failure {
    fn from(err: reqwest::Error) -> Self {
        Failure::Request(err)
    }
}

impl Failure {
    pub(crate) fn into_error(self, kind: ErrorKind) -> ProtocolError {
        match self {
            Failure::Request(err) => request_error(kind, err),
            Failure::TimedOut => ProtocolError::new(kind, anyhow!(NO_RESPONSE)),
        }
    }
}

pub(crate) fn request_error(kind: ErrorKind, err: reqwest::Error) -> ProtocolError {
    if err.is_timeout() {
        return ProtocolError::new(kind, anyhow!(NO_RESPONSE));
    }
    let err = err.without_url();
    let mut message = err.to_string();
    let mut cause = err.source();
    while let Some(inner) = cause {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        cause = inner.source();
    }
    ProtocolError::new(kind, anyhow!(message))
}

#[cfg(test)]
mod tests;
