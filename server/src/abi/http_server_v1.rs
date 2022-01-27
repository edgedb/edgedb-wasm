wit_bindgen_wasmtime::import!({
    paths: ["./wit/edgedb_http_server_v1.wit"],
    async: *,
});

pub use edgedb_http_server_v1::EdgedbHttpServerV1 as Handler;
pub use edgedb_http_server_v1::EdgedbHttpServerV1Data as State;
pub use edgedb_http_server_v1::{Request, Response};

pub struct ConvertRequest<'a> {
    hyper: &'a hyper::Request<hyper::Body>,
    headers: Vec<(&'a [u8], &'a [u8])>,
    body: hyper::body::Bytes,
}

impl<'a> ConvertRequest<'a> {
    pub async fn read_full(req: &'a mut hyper::Request<hyper::Body>)
        -> hyper::Result<ConvertRequest<'a>>
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
    pub fn as_request(&self) -> Request<'_> {
        // TODO(tailhook) uri that contains authority is skipped here
        Request {
            method: self.hyper.method().as_str(),
            uri: self.hyper.uri().path_and_query()
                .map(|p| p.as_str()).unwrap_or("/"),
            headers: &self.headers,
            body: &self.body.as_ref(),
        }
    }
}

impl TryInto<hyper::Response<hyper::Body>> for Response {
    type Error = hyper::http::Error;
    fn try_into(self) -> hyper::http::Result<hyper::Response<hyper::Body>> {
        let mut resp = hyper::Response::builder();
        resp = resp.status(self.status_code);
        for (n, v) in self.headers {
            resp = resp.header(n, v);
        }
        resp.body(self.body.into())
    }
}
