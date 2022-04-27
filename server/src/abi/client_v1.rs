wit_bindgen_wasmtime::export!({
    paths: ["../wit/edgedb-client-v1.wit"],
    async: *,
});

use std::sync::Arc;

pub use edgedb_tokio::raw::{Pool, Connection};
use edgedb_errors::{ErrorKind, ClientError};
use edgedb_protocol::common::{Cardinality};
use edgedb_protocol::common::{CompilationFlags, Capabilities, IoFormat};
use tokio::sync::Mutex;

pub use edgedb_client_v1 as v1;
pub use edgedb_client_v1::add_to_linker;
pub use edgedb_client_v1::EdgedbClientV1Tables as Tables;

use std::default::Default;

use bytes::Bytes;

use crate::bug::{Bug, Context as _};


pub type Context<'a> = (&'a mut InnerState, &'a mut Tables<InnerState>);

pub struct State {
    inner: InnerState,
    tables: Tables<InnerState>,
}

#[derive(Debug, Clone)]
pub struct Client {
    pool: Pool,
}

#[derive(Debug)]
pub struct Query {
    connection: Arc<Mutex<Connection>>,
}

#[derive(Debug)]
pub struct Transaction {
    connection: Arc<Mutex<Connection>>,
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
        use std::error::Error;
        v1::Error {
            code: err.code(),
            messages: err.initial_message().iter().map(|&s| s)
                .chain(err.contexts())
                .map(|s| s.into())
                .collect(),
            error: err.source().map(|e| e.to_string()),
            headers: err.headers().iter()
                .map(|(&k, v)| (k, v.to_vec()))
                .collect(),
        }
    }
}

#[wit_bindgen_wasmtime::async_trait]
impl v1::EdgedbClientV1 for InnerState {
    type Client = Client;
    type Query = Query;
    type Transaction = Transaction;
    async fn client_connect(&mut self) -> Client {
        Client {
            pool: self.pool.clone(),
        }
    }
    async fn client_prepare(&mut self, me: &Client,
                            flags: v1::CompilationFlags, query: &str)
        -> Result<(Query, v1::PrepareComplete), v1::Error>
    {
        let mut connection = me.pool.acquire().await?;
        let mut flags = CompilationFlags::try_from(flags)?;
        flags.allow_capabilities &= Capabilities::MODIFICATIONS;
        let prepare = connection.prepare(&flags, query).await?;
        let prepare = v1::PrepareComplete {
            capabilities: prepare.get_capabilities()
                .wrap_bug("no capabilities received")?.try_into()?,
            cardinality: prepare.cardinality.into(),
            input_typedesc_id: prepare.input_typedesc_id.to_string(),
            output_typedesc_id: prepare.input_typedesc_id.to_string(),
        };
        let query = Query {
            connection: Arc::new(Mutex::new(connection)),
        };
        Ok((query, prepare))
    }
    async fn query_describe_data(&mut self, query: &Query)
        -> Result<v1::DataDescription, v1::Error>
    {
        let mut conn = query.connection.lock().await;
        let describe = conn.describe_data().await?;
        Ok(v1::DataDescription {
            proto: conn.proto().version_tuple(),
            result_cardinality: describe.result_cardinality.into(),
            input_typedesc_id: describe.input_typedesc_id.to_string(),
            input_typedesc: describe.input_typedesc.to_vec(),
            output_typedesc_id: describe.output_typedesc_id.to_string(),
            output_typedesc: describe.output_typedesc.to_vec(),
        })
    }
    async fn query_execute(&mut self, query: &Query, arguments: &[u8])
        -> Result<v1::Data, v1::Error>
    {
        let chunks = query.connection.lock().await
            .execute(&Bytes::copy_from_slice(arguments)).await?;
        Ok(v1::Data {
            chunks: chunks.into_iter()
                .flat_map(|data| data.data.into_iter())
                .map(|d| d.to_vec())
                .collect(),
        })
    }
    async fn client_transaction(&mut self, me: &Client)
        -> Result<Transaction, v1::Error>
    {
        let mut connection = me.pool.acquire().await?;
        connection.statement("START TRANSACTION").await?;
        let transaction = Transaction {
            connection: Arc::new(Mutex::new(connection)),
        };
        // TODO(tailhook) mark transaction as dirty
        Ok(transaction)
    }
    async fn transaction_prepare(&mut self, me: &Transaction,
                                 flags: v1::CompilationFlags, query: &str)
        -> Result<(Query, v1::PrepareComplete), v1::Error>
    {
        let mut flags = CompilationFlags::try_from(flags)?;
        flags.allow_capabilities &= Capabilities::MODIFICATIONS;
        let mut connection = me.connection.lock().await;
        let prepare = connection.prepare(&flags, query).await?;
        let prepare = v1::PrepareComplete {
            capabilities: prepare.get_capabilities()
                .wrap_bug("no capabilities received")?.try_into()?,
            cardinality: prepare.cardinality.into(),
            input_typedesc_id: prepare.input_typedesc_id.to_string(),
            output_typedesc_id: prepare.input_typedesc_id.to_string(),
        };
        let query = Query { connection: me.connection.clone() };
        Ok((query, prepare))
    }
    async fn transaction_commit(&mut self, me: &Transaction)
        -> Result<(), v1::Error>
    {
        let mut connection = me.connection.lock().await;
        connection.statement("COMMIT").await?;
        Ok(())
    }
    async fn transaction_rollback(&mut self, me: &Transaction)
        -> Result<(), v1::Error>
    {
        let mut connection = me.connection.lock().await;
        connection.statement("ROLLBACK").await?;
        Ok(())
    }
}

impl TryFrom<v1::CompilationFlags> for CompilationFlags {
    type Error = Bug;
    fn try_from(src: v1::CompilationFlags) -> Result<CompilationFlags, Bug> {
        Ok(CompilationFlags {
            implicit_limit: src.implicit_limit,
            implicit_typenames: src.implicit_typenames,
            implicit_typeids: src.implicit_typeids,
            allow_capabilities: src.allow_capabilities.try_into()?,
            explicit_objectids: src.explicit_objectids,
            io_format: src.io_format.into(),
            expected_cardinality: src.expected_cardinality.into(),
        })
    }
}

impl TryFrom<Capabilities> for v1::Capabilities {
    type Error = Bug;
    fn try_from(src: Capabilities) -> Result<v1::Capabilities, Bug> {
        let bits = src.bits().try_into()
            .wrap_bug("converting capabilities from protocol to WebAssembly")?;
        v1::Capabilities::from_bits(bits)
            .wrap_bug("converting capabilities from protocol to WebAssembly")
    }
}

impl TryFrom<v1::Capabilities> for Capabilities {
    type Error = Bug;
    fn try_from(src: v1::Capabilities) -> Result<Capabilities, Bug> {
        let bits = src.bits().try_into()
            .wrap_bug("converting capabilities from WebAssembly to protocol")?;
        Capabilities::from_bits(bits)
            .wrap_bug("converting capabilities from WebAssembly to protocol")
    }
}

impl From<v1::IoFormat> for IoFormat {
    fn from(src: v1::IoFormat) -> IoFormat {
        match src {
            v1::IoFormat::Binary => IoFormat::Binary,
            v1::IoFormat::Json => IoFormat::Json,
            v1::IoFormat::JsonElements => IoFormat::JsonElements,
        }
    }
}

impl From<v1::Cardinality> for Cardinality {
    fn from(src: v1::Cardinality) -> Cardinality {
        match src {
            v1::Cardinality::NoResult => Cardinality::NoResult,
            v1::Cardinality::AtMostOne => Cardinality::AtMostOne,
            v1::Cardinality::One => Cardinality::One,
            v1::Cardinality::Many => Cardinality::Many,
            v1::Cardinality::AtLeastOne => Cardinality::AtLeastOne,
        }
    }
}

impl From<Cardinality> for v1::Cardinality {
    fn from(src: Cardinality) -> v1::Cardinality {
        match src {
            Cardinality::NoResult => v1::Cardinality::NoResult,
            Cardinality::AtMostOne => v1::Cardinality::AtMostOne,
            Cardinality::One => v1::Cardinality::One,
            Cardinality::Many => v1::Cardinality::Many,
            Cardinality::AtLeastOne => v1::Cardinality::AtLeastOne,
        }
    }
}

impl From<crate::bug::Bug> for v1::Error {
    fn from(bug: crate::bug::Bug) -> v1::Error {
        ClientError::with_message(bug.to_string()).into()
    }
}
