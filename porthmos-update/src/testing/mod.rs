use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use flate2::{Compression, write::GzEncoder};
use futures_util::StreamExt;
use http_body_util::{BodyExt, Empty, Full, StreamBody, combinators::BoxBody};
use hyper::{
    Request, Response,
    body::{Frame, Incoming},
    header::{CONTENT_LENGTH, LOCATION, USER_AGENT},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, task::JoinHandle};

type Body = BoxBody<Bytes, Infallible>;

pub(crate) enum Route {
    Body(Vec<u8>),
    Status(u16),
    Redirect(String),
    Hang,
    Endless,
    Chunked(Vec<u8>),
    LargeContentLength(u64),
}

#[derive(Default)]
struct Seen {
    user_agents: Vec<String>,
    header_names: Vec<String>,
    requests: usize,
}

pub(crate) struct TestServer {
    address: SocketAddr,
    seen: Arc<Mutex<Seen>>,
    task: JoinHandle<()>,
}

fn endless() -> Body {
    let chunks = futures_util::stream::repeat(Bytes::from(vec![b'x'; 1024]));
    BodyExt::boxed(StreamBody::new(chunks.map(|chunk| Ok::<_, Infallible>(Frame::data(chunk)))))
}

fn chunked(bytes: Vec<u8>) -> Body {
    let frames = futures_util::stream::iter([Ok::<_, Infallible>(Frame::data(Bytes::from(bytes)))]);
    BodyExt::boxed(StreamBody::new(frames))
}

fn answer(route: Option<&Route>) -> Option<Response<Body>> {
    let builder = Response::builder();
    let response = match route {
        None => builder.status(404).body(Empty::new().boxed()),
        Some(Route::Body(bytes)) => builder.body(Full::new(Bytes::from(bytes.clone())).boxed()),
        Some(Route::Status(code)) => builder.status(*code).body(Empty::new().boxed()),
        Some(Route::Redirect(location)) => builder.status(302).header(LOCATION, location).body(Empty::new().boxed()),
        Some(Route::Endless) => builder.body(endless()),
        Some(Route::Chunked(bytes)) => builder.body(chunked(bytes.clone())),
        Some(Route::LargeContentLength(length)) => builder.header(CONTENT_LENGTH, *length).body(endless()),
        Some(Route::Hang) => return None,
    };
    response.ok()
}

async fn handle(
    request: Request<Incoming>, routes: Arc<HashMap<String, Route>>, seen: Arc<Mutex<Seen>>,
) -> Result<Response<Body>, Infallible> {
    {
        let mut seen = seen.lock().unwrap();
        seen.requests += 1;
        seen.user_agents.push(
            request.headers().get(USER_AGENT).and_then(|value| value.to_str().ok()).unwrap_or_default().to_string(),
        );
        for name in request.headers().keys() {
            let name = name.as_str().to_string();
            if !seen.header_names.contains(&name) {
                seen.header_names.push(name);
            }
        }
    }
    match answer(routes.get(request.uri().path())) {
        Some(response) => Ok(response),
        None => std::future::pending().await,
    }
}

impl TestServer {
    pub(crate) async fn start(routes: Vec<(&'static str, Route)>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let routes: Arc<HashMap<String, Route>> =
            Arc::new(routes.into_iter().map(|(path, route)| (path.to_string(), route)).collect());
        let seen = Arc::new(Mutex::new(Seen::default()));
        let shared = seen.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let (routes, seen) = (routes.clone(), shared.clone());
                tokio::spawn(async move {
                    let service = service_fn(move |request| handle(request, routes.clone(), seen.clone()));
                    let _ = http1::Builder::new().serve_connection(TokioIo::new(stream), service).await;
                });
            }
        });
        TestServer { address, seen, task }
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    pub(crate) fn user_agents(&self) -> Vec<String> {
        self.seen.lock().unwrap().user_agents.clone()
    }

    pub(crate) fn header_names(&self) -> Vec<String> {
        self.seen.lock().unwrap().header_names.clone()
    }

    pub(crate) fn request_count(&self) -> usize {
        self.seen.lock().unwrap().requests
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) fn raw_archive(members: &[(&str, &[u8], tar::EntryType, u32)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (name, bytes, kind, mode) in members {
        let mut header = tar::Header::new_gnu();
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(*kind);
        header.set_size(if *kind == tar::EntryType::Regular { bytes.len() as u64 } else { 0 });
        header.set_mode(*mode);
        if *kind == tar::EntryType::Symlink {
            header.set_link_name(String::from_utf8_lossy(bytes).as_ref()).unwrap();
        }
        header.set_cksum();
        let content: &[u8] = if *kind == tar::EntryType::Regular { bytes } else { &[] };
        builder.append(&header, content).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

pub(crate) fn archive_with(members: &[(&str, &[u8])]) -> Vec<u8> {
    let members: Vec<(&str, &[u8], tar::EntryType, u32)> =
        members.iter().map(|(name, bytes)| (*name, *bytes, tar::EntryType::Regular, 0o644)).collect();
    raw_archive(&members)
}

pub(crate) fn stub_script(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\necho \"porthmos {version}\"\n").into_bytes()
}

pub(crate) fn stub_archive(version: &str, extra_members: &[(&str, &[u8])]) -> Vec<u8> {
    let script = stub_script(version);
    let mut members: Vec<(&str, &[u8], tar::EntryType, u32)> =
        vec![("porthmos", &script, tar::EntryType::Regular, 0o755)];
    members.extend(extra_members.iter().map(|(name, bytes)| (*name, *bytes, tar::EntryType::Regular, 0o644)));
    raw_archive(&members)
}

pub(crate) fn zip_archive_with(members: &[(&str, &[u8])]) -> Vec<u8> {
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in members {
        if name.ends_with('/') {
            writer.add_directory(*name, options).unwrap();
        } else {
            writer.start_file(*name, options).unwrap();
            std::io::Write::write_all(&mut writer, bytes).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}
