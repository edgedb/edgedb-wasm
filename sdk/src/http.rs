use std::str::FromStr;

use anyhow::Context;
use http_types::{Request, Method, Url};
use http_types::headers::HeaderName;

use crate::bug::{BugContext};


#[derive(minicbor::Decode)]
pub struct RawRequest<'a> {
    #[b(0)]
    method: &'a str,
    #[b(1)]
    url: &'a str,
    #[n(2)]
    headers: Vec<(&'a str, &'a str)>,
}

impl TryFrom<RawRequest<'_>> for Request {
    // TODO(tailhook) this should return bug directly
    // but for that we have to implement contexts in the bug directly
    type Error = anyhow::Error;

    fn try_from(raw: RawRequest<'_>) -> anyhow::Result<Request> {
        raw.make_req().context("cannot parse incoming HTTP request")
    }
}

impl RawRequest<'_> {
    fn make_req(self) -> anyhow::Result<Request> {
        let method = Method::from_str(&self.method)
            .wrap_bug().context("invalid method")?;
        let url = Url::from_str(&self.url)
            .wrap_bug().context("invalid url")?;
        let mut req = Request::new(method, url);

        for (name, value) in self.headers.into_iter() {
            let name = HeaderName::from_str(name)
                .wrap_bug().context("invalid header")?;
            req.append_header(name, value);
        }
        Ok(req)
    }
}
