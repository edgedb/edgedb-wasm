
use edgedb_protocol::model::Json;
use edgedb_protocol::QueryResult;
use edgedb_protocol::query_arg::QueryArgs;
use edgedb_errors::{SHOULD_RETRY};
use edgedb_errors::{NoDataError};

use crate::client::v1;
use crate::client::{Client, Error, ErrorKind};
use crate::client::{StartQuery, execute_query, execute_query_single};
use crate::client::{execute_query_json, execute_query_single_json};

// TODO(tailhook) temporary
const MAX_ITERATIONS: u32 = 3;

/// Transaction object passed to the closure via
/// [`Client::transaction()`](crate::client::Client::transaction) method
///
/// All database queries in transaction should be executed using methods on
/// this object instead of using original [`Client`](crate::client::Client)
/// instance.
#[derive(Debug)]
pub struct Transaction {
    iteration: u32,
    client: Client,
    transaction: Option<v1::Transaction>,
}

pub(crate) fn transaction<T, F>(cli: &Client, mut body: F)
    -> Result<T, Error>
        where F: FnMut(&mut Transaction) -> Result<T, Error>,
{
    let mut tx = Transaction {
        iteration: 0,
        client: cli.clone(),
        transaction: None,
    };
    'transaction: loop {
        let result = body(&mut tx);
        match result {
            Ok(val) => {
                log::debug!("Comitting transaction");
                if let Some(tx) = tx.transaction.take() {
                    tx.commit().map_err(|e| e.into_err())?;
                }
                return Ok(val)
            }
            Err(e) => {
                log::debug!("Rolling back transaction on error");
                if let Some(tx) = tx.transaction.take() {
                    tx.rollback().map_err(|e| e.into_err())?;
                }
                for e in e.chain() {
                    if let Some(e) = e.downcast_ref::<Error>() {
                        if e.has_tag(SHOULD_RETRY) {
                            if tx.iteration < MAX_ITERATIONS { // TODO
                                log::info!("Retrying transaction on {:#}",
                                           e);
                                tx.iteration += 1;
                                continue 'transaction;
                            }
                        }
                    }
                }
                return Err(e);
            }
        }
    }
}

impl StartQuery for &'_ mut Transaction {
    fn prepare(self, flags: v1::CompilationFlags, query: &str)
        -> Result<(v1::Query, v1::PrepareComplete), v1::Error>
    {
        if self.transaction.is_none() {
            self.transaction = Some(self.client.client.transaction()?);
        }
        self.transaction.as_mut().unwrap().prepare(flags, query)
    }
}

impl Transaction {
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
    pub fn query<R, A>(&mut self, query: &str, arguments: &A)
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
    pub fn query_single<R, A>(&mut self, query: &str, arguments: &A)
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
    /// [`ResultCardinalityMismatchError`][crate::errors::ResultCardinalityMismatchError]
    /// is raised. If the query returns an empty set, a
    /// [`NoDataError`][crate::errors::NoDataError] is raised.
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
    pub fn query_required_single<R, A>(&mut self,
                                             query: &str, arguments: &A)
        -> Result<R, Error>
        where A: QueryArgs,
              R: QueryResult,
    {
        self.query_single(query, arguments)?
            .ok_or_else(|| NoDataError::with_message(
                        "query row returned zero results"))
    }

    /// Execute a query and return the result as JSON.
    pub fn query_json(&mut self, query: &str, arguments: &impl QueryArgs)
        -> Result<Json, Error>
    {
        execute_query_json(self, query, arguments)
    }

    /// Execute a query and return a single result as JSON.
    ///
    /// The query must return exactly one element. If the query returns more
    /// than one element, a
    /// [`ResultCardinalityMismatchError`][crate::errors::ResultCardinalityMismatchError]
    /// is raised.
    pub fn query_single_json(&mut self,
                                   query: &str, arguments: &impl QueryArgs)
        -> Result<Option<Json>, Error>
    {
        execute_query_single_json(self, query, arguments)
    }

    /// Execute a query and return a single result as JSON.
    ///
    /// The query must return exactly one element. If the query returns more
    /// than one element, a
    /// [`ResultCardinalityMismatchError`][crate::errors::ResultCardinalityMismatchError]
    /// is raised. If the query returns an empty set, a
    /// [`NoDataError`][crate::errors::NoDataError] is raised.
    pub fn query_required_single_json(&mut self,
        query: &str, arguments: &impl QueryArgs)
        -> Result<Json, Error>
    {
        self.query_single_json(query, arguments)?
            .ok_or_else(|| NoDataError::with_message(
                        "query row returned zero results"))
    }
}
