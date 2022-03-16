wit_bindgen_wasmtime::import!({
    paths: ["./wit/edgedb_http_server_v1.wit"],
    async: *,
});

pub use edgedb_http_server_v1::EdgedbHttpServerV1 as Handler;
pub use edgedb_http_server_v1::EdgedbHttpServerV1Data as State;
pub use edgedb_http_server_v1::{Request, Response};
