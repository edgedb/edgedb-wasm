use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;

use bytes::Bytes;
use hyper::Uri;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::tenant::Tenant;
use crate::tenant::http;
use crate::abi::http_server_v1 as v1;


pub struct Process<'a>(PhantomData<&'a ()>);

#[derive(serde::Deserialize, Debug)]
pub struct Request {
    method: String,
    url: String,
    headers: Vec<(Bytes, Bytes)>,
    body: Option<Bytes>,
}

#[derive(Debug)]
pub struct ConvertRequest<'a> {
    uri: Uri,
    request: &'a Request,
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
    type Input = &'a Request;
    type ConvertInput = ConvertRequest<'a>;
    type Output = Response;

    async fn read_full(request: &'a Request)
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

async fn process_request(mut sock: UnixStream, tenant: Tenant)
    -> anyhow::Result<()>
{
    let mut request_data = Vec::with_capacity(1024);
    sock.read_to_end(&mut request_data).await?;
    let request = serde_pickle::from_slice(&request_data,
           serde_pickle::DeOptions::new().replace_unresolved_globals())?;
    let response = tenant.handle::<Process>(&request).await?;
    sock.write_all(
        &serde_pickle::to_vec(&response, serde_pickle::SerOptions::new())?
    ).await?;
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
