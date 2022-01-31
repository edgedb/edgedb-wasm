use crate::web::{self, WEB_HANDLER};

wit_bindgen_rust::export!("./wit/edgedb_http_server_v1.wit");

use edgedb_http_server_v1 as v1;

struct EdgedbHttpServerV1 {
}

impl v1::EdgedbHttpServerV1 for EdgedbHttpServerV1 {
    fn handle_request(req: v1::Request) -> v1::Response {
        if let Some(handler) = WEB_HANDLER.get() {
            let mut bld = http::Request::builder();
            bld = bld.method(&req.method[..]);
            bld = bld.uri(&req.uri);
            for (k, v) in req.headers {
                bld = bld.header(k, v);
            }
            let inner = bld.body(req.body).expect("can build request");

            let resp = handler(web::Request {
                inner,
            });

            v1::Response {
                status_code: resp.status().as_u16(),
                headers: resp.headers().iter().map(|(key, val)| {
                    (key.as_str().as_bytes().to_vec(), val.as_bytes().to_vec())
                }).collect(),
                body: resp.into_body(),
            }
        } else {
            v1::Response {
                status_code: 404,
                // TODO(tailhook) only in debug mode
                headers: vec![],
                body: b"Page Not Found (Web handler is not set)".to_vec(),
            }
        }
    }
}
