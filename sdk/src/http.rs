use anyhow::Context;
use http::request::{self, Request};
use minicbor::bytes::ByteSlice;

use crate::bug::{BugContext};


type Body = ();


#[derive(minicbor::Decode)]
pub struct RawRequest<'a> {
    #[b(0)]
    method: &'a str,
    #[b(1)]
    uri: &'a str,
    #[n(2)]
    headers: Vec<(&'a ByteSlice, &'a ByteSlice)>,
}

impl TryFrom<RawRequest<'_>> for Request<Body> {
    // TODO(tailhook) this should return bug directly
    // but for that we have to implement contexts in the bug directly
    type Error = anyhow::Error;

    fn try_from(raw: RawRequest<'_>) -> anyhow::Result<Request<Body>> {
        raw.make_req().context("cannot parse incoming HTTP request")
    }
}

impl RawRequest<'_> {
    fn make_req(&self) -> anyhow::Result<Request<Body>> {
        let mut req = request::Builder::new();
        req = req.uri(self.uri);
        req = req.method(self.method);
        for (name, value) in self.headers.iter() {
            req = req.header(&name[..], &value[..]);
        }
        Ok(req.body(()).wrap_bug().context("invalid body")?)
    }
}
