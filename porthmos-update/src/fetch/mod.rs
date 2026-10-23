use std::{fmt, time::Duration};

use anyhow::Result;
use reqwest::{Url, redirect::Policy};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FetchError {
    NotHttps,
    Status(u16),
    TooLarge { limit: usize },
    Network(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::NotHttps => f.write_str("the address does not start with https://"),
            FetchError::Status(code) => write!(f, "the server answered {code}"),
            FetchError::TooLarge { limit } => write!(f, "the answer is larger than {limit} bytes"),
            FetchError::Network(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for FetchError {}

pub(crate) struct Fetcher {
    client: reqwest::Client,
    https_only: bool,
}

fn redirect_allowed(https_only: bool, next: &Url, hops: usize) -> bool {
    hops < MAX_REDIRECTS && (!https_only || next.scheme() == "https")
}

fn classify(error: &reqwest::Error) -> FetchError {
    let text = if error.is_timeout() {
        "the request timed out"
    } else if error.is_connect() {
        "couldn't connect to the server"
    } else if error.is_redirect() {
        "the server redirected too often or somewhere not allowed"
    } else {
        "the request failed"
    };
    FetchError::Network(text.to_string())
}

fn starts_with_https(url: &str) -> bool {
    url.get(..8).is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
}

impl Fetcher {
    pub(crate) fn new(user_agent: &str, timeout: Duration, https_only: bool) -> Result<Fetcher> {
        let policy = Policy::custom(move |attempt| {
            if redirect_allowed(https_only, attempt.url(), attempt.previous().len()) {
                attempt.follow()
            } else {
                attempt.error("redirect refused")
            }
        });
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(timeout)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(policy)
            .build()?;
        Ok(Fetcher { client, https_only })
    }

    pub(crate) async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, FetchError> {
        if self.https_only && !starts_with_https(url) {
            return Err(FetchError::NotHttps);
        }
        let mut response = self.client.get(url).send().await.map_err(|error| classify(&error))?;
        if !response.status().is_success() {
            return Err(FetchError::Status(response.status().as_u16()));
        }
        if response.content_length().is_some_and(|length| length > limit as u64) {
            return Err(FetchError::TooLarge { limit });
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| classify(&error))? {
            if body.len() + chunk.len() > limit {
                return Err(FetchError::TooLarge { limit });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests;
