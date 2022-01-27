wit_bindgen_rust::export!("./wit/edgedb_http_server_v1.wit");

use edgedb_http_server_v1 as v1;

struct EdgedbHttpServerV1 {
}

impl v1::EdgedbHttpServerV1 for EdgedbHttpServerV1 {
    fn handle_request(req: v1::Request) -> v1::Response {
        v1::Response {
            status_code: 200,
            headers: vec![(b"Content-Type".to_vec(), b"text/html".to_vec())],
            body: b"Hello <b>world</b>!".to_vec(),
        }
    }
}
