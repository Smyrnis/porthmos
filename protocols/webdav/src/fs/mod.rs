use std::{io, sync::Arc};

use anyhow::anyhow;
use bytes::Bytes;
use futures_util::TryStreamExt;
use porthmos_vfs::{
    DirItem, Entry, ErrorKind, FileSystem, Metadata, ProtocolError, Reader, VfsPath, Writer, async_trait,
};
use reqwest::{
    Body, Method, RequestBuilder, Response, StatusCode,
    header::{CONTENT_TYPE, RANGE},
};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::io::{ReaderStream, StreamReader};

use crate::{
    client::{DavClient, method},
    errors::{Failure, request_error, status_error},
    idle::IdleReader,
    paths::normalize,
    propfind::{PROPFIND_BODY, Resource, XML_CONTENT_TYPE, parse_multistatus},
    upload::{PATCH_CHUNK, PIPE_CAPACITY, PatchUpload, PutUpload, SendChunk, Verify, Waits},
};

const SABRE_PARTIAL_UPDATE: &str = "application/x-sabredav-partialupdate";

pub(crate) struct WebDavFs {
    client: Arc<DavClient>,
    partial_update: bool,
    info: Option<String>,
}

fn other(failure: Failure) -> ProtocolError {
    failure.into_error(ErrorKind::Other)
}

fn body_reader(client: &DavClient, response: Response) -> IdleReader<impl AsyncRead + Unpin + use<>> {
    let stream = response.bytes_stream().map_err(|err| io::Error::other(err.without_url()));
    IdleReader::new(StreamReader::new(Box::pin(stream)), client.timeout())
}

pub(crate) async fn propfind(
    client: &DavClient, path: &VfsPath, collection: bool, depth: &'static str,
) -> Result<Response, Failure> {
    let href = client.locator().href(path, collection);
    client
        .send(method("PROPFIND"), &href, |builder| {
            builder.header("Depth", depth).header(CONTENT_TYPE, XML_CONTENT_TYPE).body(PROPFIND_BODY)
        })
        .await
}

async fn multistatus(client: &DavClient, response: Response, path: &VfsPath) -> Result<Vec<Resource>, ProtocolError> {
    if response.status() != StatusCode::MULTI_STATUS {
        return Err(status_error(response.status(), response.headers(), path));
    }
    let mut body = Vec::new();
    body_reader(client, response).read_to_end(&mut body).await?;
    parse_multistatus(&String::from_utf8_lossy(&body))
}

async fn stat_resource(client: &DavClient, path: &VfsPath) -> Result<Resource, ProtocolError> {
    let mut response = propfind(client, path, false, "0").await.map_err(other)?;
    if response.status().is_redirection() {
        response = propfind(client, path, true, "0").await.map_err(other)?;
    }
    multistatus(client, response, path)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| ProtocolError::new(ErrorKind::NotFound, anyhow!("{} not found", path)))
}

fn expect_success(response: &Response, path: &VfsPath) -> Result<(), ProtocolError> {
    if response.status().is_success() { Ok(()) } else { Err(status_error(response.status(), response.headers(), path)) }
}

async fn finish_put(client: Arc<DavClient>, request: RequestBuilder, path: VfsPath) -> io::Result<()> {
    let response = request.send().await.map_err(|err| io::Error::other(request_error(ErrorKind::Other, err)))?;
    if response.status() == StatusCode::UNAUTHORIZED {
        client.refresh(&response);
    }
    expect_success(&response, &path).map_err(io::Error::other)
}

impl WebDavFs {
    pub(crate) fn new(client: Arc<DavClient>, partial_update: bool, info: Option<String>) -> Self {
        Self { client, partial_update, info }
    }

    async fn move_to(
        &self, from: &VfsPath, to: &VfsPath, collection: bool, overwrite: bool,
    ) -> Result<Response, ProtocolError> {
        let locator = self.client.locator();
        let destination = locator.url(&locator.href(to, collection));
        let overwrite = if overwrite { "T" } else { "F" };
        self.client
            .send(method("MOVE"), &locator.href(from, collection), |builder| {
                builder.header("Destination", destination.as_str()).header("Overwrite", overwrite)
            })
            .await
            .map_err(other)
    }

    async fn completed(&self, response: Response, path: &VfsPath, action: &str) -> Result<(), ProtocolError> {
        if response.status() != StatusCode::MULTI_STATUS {
            return expect_success(&response, path);
        }
        let failed = multistatus(&self.client, response, path)
            .await?
            .into_iter()
            .find(|resource| resource.status.is_some_and(|status| !(200..300).contains(&status)));
        match failed {
            Some(resource) => {
                let shown =
                    self.client.locator().path_of_href(&resource.href).unwrap_or_else(|| VfsPath::new(&resource.href));
                Err(ProtocolError::new(
                    ErrorKind::Other,
                    anyhow!("{}: could not {action} {} ({})", path, shown, resource.status.unwrap_or_default()),
                ))
            }
            None => Ok(()),
        }
    }

    fn verifier(&self, path: &VfsPath) -> Verify {
        let client = self.client.clone();
        let path = path.clone();
        Box::new(move |written| {
            Box::pin(async move {
                let resource = stat_resource(&client, &path).await.map_err(io::Error::other)?;
                match resource.size {
                    Some(stored) if stored != written => {
                        Err(io::Error::other(format!("the server stored {stored} of {written} bytes")))
                    }
                    _ => Ok(()),
                }
            })
        })
    }

    fn patch_sender(&self, path: &VfsPath) -> SendChunk {
        let client = self.client.clone();
        let href = client.locator().href(path, false);
        let path = path.clone();
        Arc::new(move |start, data: Bytes| {
            let client = client.clone();
            let href = href.clone();
            let path = path.clone();
            Box::pin(async move {
                let range = format!("bytes={start}-{}", start + data.len() as u64 - 1);
                let response = client
                    .send(Method::PATCH, &href, |builder| {
                        builder
                            .header(CONTENT_TYPE, SABRE_PARTIAL_UPDATE)
                            .header("X-Update-Range", range.as_str())
                            .body(data.clone())
                    })
                    .await
                    .map_err(|failure| io::Error::other(other(failure)))?;
                expect_success(&response, &path).map_err(io::Error::other)
            })
        })
    }
}

#[async_trait]
impl FileSystem for WebDavFs {
    fn server_info(&self) -> Option<String> {
        self.info.clone()
    }

    async fn list(&self, dir: &VfsPath) -> Result<Vec<Entry>, ProtocolError> {
        Ok(self
            .read_dir(dir)
            .await?
            .into_iter()
            .map(|item| Entry {
                is_dir: item.metadata.is_dir(),
                size: item.metadata.size,
                permissions: item.metadata.permissions,
                modified: item.metadata.modified,
                name: item.name,
                path: item.path,
                ..Default::default()
            })
            .collect())
    }

    async fn read_dir(&self, dir: &VfsPath) -> Result<Vec<DirItem>, ProtocolError> {
        let dir = normalize(dir);
        let response = propfind(&self.client, &dir, true, "1").await.map_err(other)?;
        let resources = multistatus(&self.client, response, &dir).await?;
        let located: Vec<(VfsPath, Resource)> = resources
            .into_iter()
            .filter_map(|resource| Some((self.client.locator().path_of_href(&resource.href)?, resource)))
            .collect();
        if located.is_empty() {
            return Err(ProtocolError::new(
                ErrorKind::Other,
                anyhow!("the server answered with paths outside Root path {}", self.client.locator().root()),
            ));
        }
        Ok(located
            .into_iter()
            .filter(|(path, _)| *path != dir)
            .filter_map(|(path, resource)| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                Some(DirItem { name, metadata: resource.metadata(), path })
            })
            .collect())
    }

    async fn stat(&self, path: &VfsPath) -> Result<Metadata, ProtocolError> {
        Ok(stat_resource(&self.client, path).await?.metadata())
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let href = self.client.locator().href(path, true);
        let response = self.client.send(method("MKCOL"), &href, |builder| builder).await.map_err(other)?;
        if response.status() == StatusCode::METHOD_NOT_ALLOWED {
            return Err(ProtocolError::new(ErrorKind::Other, anyhow!("{} already exists", path)));
        }
        expect_success(&response, path)
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), ProtocolError> {
        let collection = stat_resource(&self.client, from).await?.collection;
        let response = self.move_to(from, to, collection, false).await?;
        if response.status() != StatusCode::PRECONDITION_FAILED {
            return self.completed(response, from, "move").await;
        }
        if collection || stat_resource(&self.client, to).await?.collection {
            return Err(ProtocolError::new(ErrorKind::Other, anyhow!("{} already exists", to)));
        }
        let response = self.move_to(from, to, collection, true).await?;
        self.completed(response, from, "move").await
    }

    async fn remove_file(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let href = self.client.locator().href(path, false);
        let response = self.client.send(Method::DELETE, &href, |builder| builder).await.map_err(other)?;
        self.completed(response, path, "delete").await
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), ProtocolError> {
        let collection = stat_resource(&self.client, path).await?.collection;
        let href = self.client.locator().href(path, collection);
        let response = self.client.send(Method::DELETE, &href, |builder| builder).await.map_err(other)?;
        self.completed(response, path, "delete").await
    }

    async fn home(&self) -> Result<VfsPath, ProtocolError> {
        Ok(VfsPath::root())
    }

    async fn open_read(&self, path: &VfsPath, offset: u64) -> Result<Reader, ProtocolError> {
        let href = self.client.locator().href(path, false);
        let response = self
            .client
            .send(Method::GET, &href, |builder| {
                if offset > 0 { builder.header(RANGE, format!("bytes={offset}-")) } else { builder }
            })
            .await
            .map_err(other)?;
        let status = response.status();
        if status == StatusCode::RANGE_NOT_SATISFIABLE && offset > 0 {
            return Ok(Box::new(tokio::io::empty()));
        }
        expect_success(&response, path)?;
        let mut reader = body_reader(&self.client, response);
        if status != StatusCode::PARTIAL_CONTENT && offset > 0 {
            let skipped = tokio::io::copy(&mut (&mut reader).take(offset), &mut tokio::io::sink()).await?;
            if skipped < offset {
                return Err(ProtocolError::new(
                    ErrorKind::Other,
                    anyhow!("the server sent fewer bytes than the resume offset"),
                ));
            }
        }
        Ok(Box::new(reader))
    }

    async fn open_write(&self, path: &VfsPath, offset: u64) -> Result<Writer, ProtocolError> {
        if offset > 0 && self.partial_update {
            let upload = PatchUpload::new(self.patch_sender(path), offset, PATCH_CHUNK);
            return Ok(Writer { stream: Box::new(upload), offset });
        }
        let href = self.client.locator().href(path, false);
        if self.client.awaits_challenge() {
            self.client.send(Method::PUT, &href, |builder| builder.body("")).await.map_err(other)?;
        }
        let (pipe, body) = tokio::io::duplex(PIPE_CAPACITY);
        let request = self.client.request(Method::PUT, &href).body(Body::wrap_stream(ReaderStream::new(body)));
        let task = tokio::spawn(finish_put(self.client.clone(), request, path.clone()));
        let waits = Waits { idle: self.client.timeout(), reply: self.client.long_wait() };
        Ok(Writer { stream: Box::new(PutUpload::new(pipe, task, self.verifier(path), waits)), offset: 0 })
    }
}
