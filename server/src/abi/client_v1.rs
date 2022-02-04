use std::default::Default;

wit_bindgen_wasmtime::export!({
    paths: ["./wit/edgedb-client-v1.wit"],
    async: *,
});

pub use edgedb_tokio::raw::Pool;

pub use edgedb_client_v1 as v1;
pub use edgedb_client_v1::add_to_linker;
pub use edgedb_client_v1::EdgedbClientV1Tables as Tables;


pub type Context<'a> = (&'a mut InnerState, &'a mut Tables<InnerState>);

pub struct State {
    inner: InnerState,
    tables: Tables<InnerState>,
}

#[derive(Debug)]
pub struct Client {
    pool: Pool,
}

#[derive(Debug)]
pub struct Query {
}

pub struct InnerState {
    pool: Pool,
}

impl State {
    pub fn new(pool: &Pool) -> State {
        State {
            inner: InnerState {
                pool: pool.clone(),
            },
            tables: Default::default(),
        }
    }

    pub fn context(&mut self) -> Context<'_> {
        (&mut self.inner, &mut self.tables)
    }
}

impl From<edgedb_tokio::Error> for v1::Error {
    fn from(err: edgedb_tokio::Error) -> v1::Error {
        dbg!(err);
        todo!();
    }
}

#[wit_bindgen_wasmtime::async_trait]
impl v1::EdgedbClientV1 for InnerState {
    type Client = Client;
    type Query = Query;
    async fn client_connect(&mut self) -> Client {
        Client {
            pool: self.pool.clone(),
        }
    }
    async fn client_prepare(&mut self, me: &Client,
                            _query: v1::PrepareQuery<'_>)
        -> Result<v1::PrepareComplete<Self>, v1::Error>
    {
        let conn = me.pool.acquire().await?;
        //self.pool.query(
        todo!();
    }
    async fn query_describe_data(&mut self, _me: &Query)
        -> Result<v1::DataDescription, v1::Error>
    {
        todo!();
    }
    async fn query_execute(&mut self, _me: &Query, _arguments: &[u8])
        -> Result<v1::Data, v1::Error>
    {
        todo!();
    }
}
