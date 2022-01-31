wit_bindgen_wasmtime::export!({
    paths: ["./wit/edgedb-client-v1.wit"],
    async: *,
});

pub use edgedb_client_v1 as v1;
pub use edgedb_client_v1::add_to_linker;
pub use edgedb_client_v1::EdgedbClientV1Tables as Tables;

pub type Context<'a> = (&'a mut InnerState, &'a mut Tables<InnerState>);

#[derive(Default)]
pub struct State {
    inner: InnerState,
    tables: Tables<InnerState>,
}

#[derive(Debug)]
pub struct Client {
}

#[derive(Default)]
pub struct InnerState {
}

impl State {
    pub fn context(&mut self) -> Context<'_> {
        (&mut self.inner, &mut self.tables)
    }
}


#[wit_bindgen_wasmtime::async_trait]
impl v1::EdgedbClientV1 for InnerState {
    type Client = Client;
    async fn client_connect(&mut self) -> Client {
        Client {
        }
    }
    async fn client_query(&mut self,
                          _me: &Client, _query: &str, _arguments: &[u8])
        -> Result<v1::Response, v1::Error>
    {
        todo!();
    }
}
