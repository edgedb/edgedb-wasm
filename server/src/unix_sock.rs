use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::path::PathBuf;

use bytes::Bytes;
use hyper::Uri;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::tenant::Tenant;
use crate::tenant::http;
use crate::abi::http_server_v1 as v1;


pub struct Process<'a>(PhantomData<&'a ()>);

#[derive(serde::Deserialize, Debug)]
#[serde(tag="request", content="params", rename_all="snake_case")]
pub enum Request {
    SetDirectory(SetDirectory),
    Http(HttpRequest),
}

// We can't use unit type instead, because we serialize `Success` as dict,
// so anything inside should be dict-like (e.g. a structure)
#[derive(serde::Serialize, Debug)]
struct PyNone {
}

#[derive(serde::Serialize, Debug)]
#[serde(tag="response", rename_all="snake_case")]
pub enum Signal<T> {
    Success(T),
    Failure { error: String },
}

impl<T: serde::Serialize, E: fmt::Display> Into<Signal<T>> for Result<T, E> {
    fn into(self) -> Signal<T> {
        match self {
            Ok(val) => Signal::Success(val),
            // TODO(tailhook) maybe hide error message?
            Err(e) => {
                log::warn!("Erroneous response: {e:#}");
                Signal::Failure { error: e.to_string() }
            }
        }
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct SetDirectory {
    database: String,
    directory: PathBuf,
}

#[derive(serde::Deserialize, Debug)]
pub struct HttpRequest {
    method: String,
    url: String,
    headers: Vec<(Bytes, Bytes)>,
    body: Option<Bytes>,
}

#[derive(Debug)]
pub struct ConvertRequest<'a> {
    uri: Uri,
    request: &'a HttpRequest,
    headers: Vec<(&'a [u8], &'a [u8])>,
}

#[derive(serde::Serialize, Debug)]
pub struct Response {
    status: u16,
    headers: HashMap<Bytes, Bytes>,
    body: Option<Bytes>,
}


#[async_trait::async_trait]
impl<'a> http::Process for Process<'a> {
    type Input = &'a HttpRequest;
    type ConvertInput = ConvertRequest<'a>;
    type Output = Response;

    async fn read_full(request: &'a HttpRequest)
        -> anyhow::Result<ConvertRequest<'a>>
    {
        Ok(ConvertRequest {
            uri: request.url.parse()?,
            request,
            headers: request.headers.iter().map(|(n, v)| {
                (n.as_ref(), v.as_ref())
            }).collect(),
        })
    }

    fn err_not_found() -> Self::Output {
        // TODO(tailhook) only in debug mode
        Response {
            status: hyper::StatusCode::NOT_FOUND.as_u16(),
            headers: HashMap::new(),
            body: Some(b"Try /db/<database>/<wasm-file-name>/"[..].into()),
        }
    }
    fn err_internal_server_error() -> Self::Output {
        // TODO(tailhook) only in debug mode
        Response {
            status: hyper::StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            headers: HashMap::new(),
            body: Some(b"Wasm failed to handle request"[..].into()),
        }
    }
}

async fn respond<T>(mut sock: UnixStream, response: impl Into<Signal<T>>)
    -> anyhow::Result<()>
    where T: serde::Serialize,
{
    let response = response.into();
    sock.write_all(
        &serde_pickle::to_vec(&response, serde_pickle::SerOptions::new())?
    ).await?;
    Ok(())
}

async fn process_request(mut sock: UnixStream, tenant: Tenant)
    -> anyhow::Result<()>
{
    let mut request_data = Vec::with_capacity(1024);
    sock.read_to_end(&mut request_data).await?;
    let request = serde_pickle::from_slice(&request_data,
           serde_pickle::DeOptions::new().replace_unresolved_globals())?;
    match request {
        Request::Http(request) => {
            respond(sock, tenant.handle::<Process>(&request).await).await?;
        }
        Request::SetDirectory(SetDirectory { database, directory }) => {
            tenant.set_directory(&database, &directory).await;
            respond(sock, Signal::Success(PyNone {})).await?;
        }
    }
    Ok(())
}

pub fn service(sock: UnixStream, tenant: &Tenant) -> impl Future<Output=()> {
    let tenant = tenant.clone();
    async move {
        process_request(sock, tenant).await.map_err(|e| {
            log::error!("Error handling request: {e:#}");
        }).ok();
    }
}

impl http::ConvertInput for ConvertRequest<'_> {
    fn uri(&self) -> &hyper::Uri {
        &self.uri
    }
    fn as_v1(&self) -> v1::Request<'_> {
        v1::Request {
            method: &self.request.method[..],
            uri: &self.request.url[..],
            headers: &self.headers,
            body: self.request.body.as_deref().unwrap_or(b""),
        }
    }
}

impl http::FromWasm for Response {
    fn from_wasm(wasm: v1::Response) -> anyhow::Result<Self> {
        Ok(Response {
            status: wasm.status_code,
            headers: wasm.headers.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            body: Some(wasm.body.into()),
        })
    }
}
