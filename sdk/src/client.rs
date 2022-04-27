use std::collections::HashMap;
use std::sync::Arc;

pub use edgedb_protocol::QueryResult;
pub use edgedb_protocol::common::Cardinality;
pub use edgedb_protocol::features::ProtocolVersion;
pub use edgedb_protocol::query_arg::{QueryArgs, Encoder};
pub use edgedb_protocol::server_message::CommandDataDescription;
pub use edgedb_errors::{Error, ErrorKind};
pub use edgedb_errors::{ClientError, ProtocolEncodingError, NoResultExpected};

use bytes::BytesMut;

wit_bindgen_rust::import!("../wit/edgedb-client-v1.wit");

mod transaction;

use edgedb_client_v1 as v1;
use transaction::{Transaction, transaction};

#[derive(Debug, Clone)]
pub struct Client {
    client: Arc<v1::Client>,
}

pub fn create_client() -> Client {
    Client {
        client: Arc::new(v1::Client::connect()),
    }
}

trait StartQuery {
    fn prepare(self, flags: v1::CompilationFlags, query: &str)
        -> Result<(v1::Query, v1::PrepareComplete), v1::Error>;
}

impl v1::Error {
    fn into_err(self) -> Error {
        let mut err = Error::from_code(self.code);
        for msg in self.messages {
            err = err.context(msg);
        }
        // TODO(tailhook) headers
        return err;
    }
}

impl StartQuery for &'_ Client {
    fn prepare(self, flags: v1::CompilationFlags, query: &str)
        -> Result<(v1::Query, v1::PrepareComplete), v1::Error>
    {
        self.client.prepare(flags, query)
    }
}

fn execute_query<T: StartQuery, R, A>(target: T, query: &str, arguments: &A)
    -> Result<Vec<R>, Error>
    where A: QueryArgs,
          R: QueryResult,
{
    let flags = v1::CompilationFlags {
        implicit_limit: None,
        implicit_typenames: false,
        implicit_typeids: false,
        explicit_objectids: true,
        // host app will remove everything else anyway
        allow_capabilities: v1::Capabilities::MODIFICATIONS,
        io_format: v1::IoFormat::Binary,
        expected_cardinality: v1::Cardinality::Many,
    };
    let (query, _prepare_info) = target.prepare(flags, query)
        .map_err(|e| e.into_err())?;
    let desc = query.describe_data().map_err(|e| e.into_err())?;
    let desc = CommandDataDescription::try_from(desc)?;
    let inp_desc = desc.input()
        .map_err(ProtocolEncodingError::with_source)?;

    let mut arg_buf = BytesMut::with_capacity(8);
    arguments.encode(&mut Encoder::new(
        &inp_desc.as_query_arg_context(),
        &mut arg_buf,
    ))?;

    let data = query.execute(&arg_buf).map_err(|e| e.into_err())?;

    let out_desc = desc.output()
        .map_err(ProtocolEncodingError::with_source)?;
    match out_desc.root_pos() {
        Some(root_pos) => {
            let ctx = out_desc.as_queryable_context();
            let mut state = R::prepare(&ctx, root_pos)?;
            let rows = data.chunks.into_iter()
               .map(|chunk| R::decode(&mut state, &chunk.into()))
               .collect::<Result<_, _>>()?;
            Ok(rows)
        }
        None => Err(NoResultExpected::build()),
    }
}

impl Client {
    pub fn query<R, A>(&self, query: &str, arguments: &A)
        -> Result<Vec<R>, Error>
        where A: QueryArgs,
              R: QueryResult,
    {
        execute_query(self, query, arguments)
    }
    /// Execute a transaction
    ///
    /// Transaction body must be encompassed in the closure. The closure **may
    /// be executed multiple times**. This includes not only database queries
    /// but also executing the whole function, so the transaction code must be
    /// prepared to be idempotent.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # fn transaction() -> Result<(), edgedb_tokio::Error> {
    /// let conn = edgedb_sdk::create_client().await?;
    /// let val = conn.transaction(|mut tx| {
    ///     // TODO(tailhook) query_required_single
    ///     tx.query::<i64, _>("
    ///         WITH C := UPDATE Counter SET { value := .value + 1}
    ///         SELECT C.value LIMIT 1
    ///     ", &()
    ///     ).remove(0)
    /// })?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn transaction<T, F>(&self, body: F) -> Result<T, Error>
        where F: FnMut(&mut Transaction) -> Result<T, Error>,
    {
        transaction(&self, body)
    }
}

impl TryFrom<v1::DataDescription> for CommandDataDescription {
    type Error = Error;
    fn try_from(src: v1::DataDescription)
        -> Result<CommandDataDescription, Error>
    {
        Ok(CommandDataDescription {
            proto: ProtocolVersion::new(src.proto.0, src.proto.1),
            headers: HashMap::new(),
            result_cardinality: src.result_cardinality.into(),
            input_typedesc_id: src.input_typedesc_id.parse()
                .map_err(ClientError::with_source)?,
            input_typedesc: src.input_typedesc.into(),
            output_typedesc_id: src.output_typedesc_id.parse()
                .map_err(ClientError::with_source)?,
            output_typedesc: src.output_typedesc.into(),
        })
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
