pub use edgedb_protocol::QueryResult;
pub use edgedb_protocol::query_arg::QueryArgs;
pub use edgedb_errors::Error;

wit_bindgen_rust::import!("./wit/edgedb-client-v1.wit");

use edgedb_client_v1 as v1;

pub struct Client {
    client: v1::Client,
}

pub fn connect() -> Client {
    Client {
        client: v1::Client::connect(),
    }
}

impl Into<Error> for v1::Error {
    fn into(self) -> Error {
        todo!();
    }
}

impl Client {
    pub fn query<R, A>(&self, request: &str, arguments: &A)
        -> Result<Vec<R>, Error>
        where A: QueryArgs,
              R: QueryResult,
    {
        let prepare = v1::PrepareQuery {
            implicit_limit: None,
            implicit_typenames: false,
            implicit_typeids: false,
            explicit_objectids: true,
            // host app will remove everything else anyway
            allow_capabilities: v1::Capabilities::MODIFICATIONS,
            io_format: v1::IoFormat::Binary,
            expected_cardinality: v1::Cardinality::Many,
            command: request,
        };
        //let request_bytes = Vec::new();
        // TODO(tailhook) serialize arguments
        let resp_bytes = self.client.prepare(prepare)
            .map_err(|e| e.into())?;
        todo!();
    }
}
