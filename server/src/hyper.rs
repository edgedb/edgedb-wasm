use std::marker::PhantomData;

use hyper::Uri;

use crate::tenant::http;
use crate::abi::http_server_v1 as v1;


pub struct Process<'a>(PhantomData<&'a ()>);

#[derive(Debug)]
pub struct ConvertRequest<'a> {
    hyper: &'a hyper::Request<hyper::Body>,
    headers: Vec<(&'a [u8], &'a [u8])>,
    body: hyper::body::Bytes,
}

#[async_trait::async_trait]
impl<'a> http::Process for Process<'a> {
    type Input = &'a mut hyper::Request<hyper::Body>;
    type ConvertInput = ConvertRequest<'a>;
    type Output = hyper::Response<hyper::Body>;

    async fn read_full(req: &'a mut hyper::Request<hyper::Body>)
        -> anyhow::Result<ConvertRequest<'a>>
    {
        let body = hyper::body::to_bytes(req.body_mut()).await?;
        let headers = req.headers().iter().map(|(n, v)| {
            (n.as_ref(), v.as_bytes())
        }).collect();
        Ok(ConvertRequest {
            hyper: req,
            headers,
            body,
        })
    }
    fn err_not_found() -> Self::Output {
        // TODO(tailhook) only in debug mode
        hyper::Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body(b"Try /db/default/<wasm-file-name>/"[..].into())
            .expect("can compose static response")
    }
    fn err_internal_server_error() -> Self::Output {
        // TODO(tailhook) only in debug mode
        hyper::Response::builder()
            .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
            .body(b"Wasm failed to handle request"[..].into())
            .expect("can compose static response")
    }
}

impl<'a> http::ConvertInput for ConvertRequest<'a> {
    fn uri(&self) -> &Uri {
        self.hyper.uri()
    }
    fn as_v1(&self) -> v1::Request<'_> {
        // TODO(tailhook) uri that contains authority is skipped here
        v1::Request {
            method: self.hyper.method().as_str(),
            uri: self.hyper.uri().path_and_query()
                .map(|p| p.as_str()).unwrap_or("/"),
            headers: &self.headers,
            body: &self.body.as_ref(),
        }
    }
}

impl http::FromWasm for hyper::Response<hyper::Body> {
    fn from_wasm(req: v1::Response)
        -> anyhow::Result<hyper::Response<hyper::Body>>
    {
        let mut resp = hyper::Response::builder();
        resp = resp.status(req.status_code);
        for (n, v) in req.headers {
            resp = resp.header(n, v);
        }
        Ok(resp.body(req.body.into())?)
    }
}
