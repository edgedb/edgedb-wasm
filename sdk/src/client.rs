//! Client to the EdgeDB
//!
//! This is a major way to contact the database. Database credentials always
//! come preconfigured to connect to the specific database that this WebAssembly
//! file was run from.
use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;

pub use edgedb_errors::{self as errors, Error, ErrorKind};
pub use edgedb_protocol::QueryResult;
pub use edgedb_protocol::common::Cardinality;
pub use edgedb_protocol::features::ProtocolVersion;
pub use edgedb_protocol::query_arg::{QueryArgs, Encoder};
pub use edgedb_protocol::server_message::CommandDataDescription;
use edgedb_errors::{ClientError, ProtocolEncodingError, NoResultExpected};
use edgedb_errors::{NoDataError};
use edgedb_protocol::model::Json;

use bytes::BytesMut;

wit_bindgen_rust::import!("../wit/edgedb-client-v1.wit");

mod transaction;

use edgedb_client_v1 as v1;
use transaction::{Transaction, transaction};

/// EdgeDB Client
///
/// Internally it contains a connection pool.
///
/// To create client, use [`create_client`] function.
#[derive(Debug, Clone)]
pub struct Client {
    client: Arc<v1::Client>,
}

/// Create a connection to the database that this WebAssembly app is attached to
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

fn execute_query_single<T: StartQuery, R, A>(target: T,
                                             query: &str, arguments: &A)
    -> Result<Option<R>, Error>
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
        expected_cardinality: v1::Cardinality::AtMostOne,
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
            let bytes = data.chunks.into_iter().next();
            if let Some(bytes) = bytes {
                Ok(Some(R::decode(&mut state, &Bytes::from(bytes))?))
            } else {
                Ok(None)
            }
        }
        None => Err(NoResultExpected::build()),
    }
}

fn execute_query_json<T: StartQuery>(target: T,
                                     query: &str, arguments: &impl QueryArgs)
    -> Result<Json, Error>
{
    let flags = v1::CompilationFlags {
        implicit_limit: None,
        implicit_typenames: false,
        implicit_typeids: false,
        explicit_objectids: true,
        // host app will remove everything else anyway
        allow_capabilities: v1::Capabilities::MODIFICATIONS,
        io_format: v1::IoFormat::Json,
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
            // JSON objects are returned as strings :(
            let mut state = String::prepare(&ctx, root_pos)?;
            let bytes = data.chunks.into_iter().next();
            if let Some(bytes) = bytes {
                // we trust database to produce valid json
                let s = String::decode(&mut state, &Bytes::from(bytes))?;
                Ok(unsafe { Json::new_unchecked(s) })
            } else {
                Err(NoDataError::with_message(
                    "query row returned zero results"))
            }
        }
        None => Err(NoResultExpected::build()),
    }
}

fn execute_query_single_json<T: StartQuery>(target: T,
    query: &str, arguments: &impl QueryArgs)
    -> Result<Option<Json>, Error>
{
    let flags = v1::CompilationFlags {
        implicit_limit: None,
        implicit_typenames: false,
        implicit_typeids: false,
        explicit_objectids: true,
        // host app will remove everything else anyway
        allow_capabilities: v1::Capabilities::MODIFICATIONS,
        io_format: v1::IoFormat::Json,
        expected_cardinality: v1::Cardinality::AtMostOne,
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
            // JSON objects are returned as strings :(
            let mut state = String::prepare(&ctx, root_pos)?;
            let bytes = data.chunks.into_iter().next();
            if let Some(bytes) = bytes {
                // we trust database to produce valid json
                let s = String::decode(&mut state, &Bytes::from(bytes))?;
                Ok(Some(unsafe { Json::new_unchecked(s) }))
            } else {
                Ok(None)
            }
        }
        None => Err(NoResultExpected::build()),
    }
}

impl Client {
    /// Execute a query and return a collection of results.
    ///
    /// You will usually have to specify the return type for the query:
    ///
    /// ```rust,ignore
    /// let greeting = pool.query::<String, _>("SELECT 'hello'", &());
    /// // or
    /// let greeting: Vec<String> = pool.query("SELECT 'hello'", &());
    /// ```
    ///
    /// This method can be used with both static arguments, like a tuple of
    /// scalars, and with dynamic arguments [`edgedb_protocol::value::Value`].
    /// Similarly, dynamically typed results are also supported.
    pub fn query<R, A>(&self, query: &str, arguments: &A)
        -> Result<Vec<R>, Error>
        where A: QueryArgs,
              R: QueryResult,
    {
        execute_query(self, query, arguments)
    }

    /// Execute a query and return a single result
    ///
    /// You will usually have to specify the return type for the query:
    ///
    /// ```rust,ignore
    /// let greeting = pool.query_single::<String, _>("SELECT 'hello'", &());
    /// // or
    /// let greeting: Option<String> = pool.query_single(
    ///     "SELECT 'hello'",
    ///     &()
    /// );
    /// ```
    ///
    /// This method can be used with both static arguments, like a tuple of
    /// scalars, and with dynamic arguments [`edgedb_protocol::value::Value`].
    /// Similarly, dynamically typed results are also supported.
    pub fn query_single<R, A>(&self, query: &str, arguments: &A)
        -> Result<Option<R>, Error>
        where A: QueryArgs,
              R: QueryResult,
    {
        execute_query_single(self, query, arguments)
    }

    /// Execute a query and return a single result
    ///
    /// The query must return exactly one element. If the query returns more
    /// than one element, a
    /// [`ResultCardinalityMismatchError`][crate::client::errors::ResultCardinalityMismatchError]
    /// is raised. If the query returns an empty set, a
    /// [`NoDataError`][crate::client::errors::NoDataError] is raised.
    ///
    /// You will usually have to specify the return type for the query:
    ///
    /// ```rust,ignore
    /// let greeting = pool.query_required_single::<String, _>(
    ///     "SELECT 'hello'",
    ///     &(),
    /// );
    /// // or
    /// let greeting: String = pool.query_required_single(
    ///     "SELECT 'hello'",
    ///     &(),
    /// );
    /// ```
    ///
    /// This method can be used with both static arguments, like a tuple of
    /// scalars, and with dynamic arguments [`edgedb_protocol::value::Value`].
    /// Similarly, dynamically typed results are also supported.
    pub fn query_required_single<R, A>(&self, query: &str, arguments: &A)
        -> Result<R, Error>
        where A: QueryArgs,
              R: QueryResult,
    {
        self.query_single(query, arguments)?
            .ok_or_else(|| NoDataError::with_message(
                        "query row returned zero results"))
    }

    /// Execute a query and return the result as JSON.
    pub fn query_json(&self, query: &str, arguments: &impl QueryArgs)
        -> Result<Json, Error>
    {
        execute_query_json(self, query, arguments)
    }

    /// Execute a query and return a single result as JSON.
    ///
    /// The query must return exactly one element. If the query returns more
    /// than one element, a
    /// [`ResultCardinalityMismatchError`][crate::client::errors::ResultCardinalityMismatchError]
    /// is raised.
    pub fn query_single_json(&self,
                                   query: &str, arguments: &impl QueryArgs)
        -> Result<Option<Json>, Error>
    {
        execute_query_single_json(self, query, arguments)
    }

    /// Execute a query and return a single result as JSON.
    ///
    /// The query must return exactly one element. If the query returns more
    /// than one element, a
    /// [`ResultCardinalityMismatchError`][crate::client::errors::ResultCardinalityMismatchError]
    /// is raised. If the query returns an empty set, a
    /// [`NoDataError`][crate::client::errors::NoDataError] is raised.
    pub fn query_required_single_json(&self,
                                   query: &str, arguments: &impl QueryArgs)
        -> Result<Json, Error>
    {
        self.query_single_json(query, arguments)?
            .ok_or_else(|| NoDataError::with_message(
                        "query row returned zero results"))
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
    /// # fn transaction() -> Result<(), edgedb_sdk::client::Error> {
    /// let conn = edgedb_sdk::client::create_client();
    /// let val = conn.transaction(|mut tx| {
    ///     // TODO(tailhook) query_required_single
    ///     tx.query_required_single::<i64, _>("
    ///         WITH C := UPDATE Counter SET { value := .value + 1}
    ///         SELECT C.value LIMIT 1
    ///     ", &()
    ///     )
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
