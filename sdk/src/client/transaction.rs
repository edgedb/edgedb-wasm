use crate::client::v1;
use crate::client::{Client, Error};
use edgedb_protocol::QueryResult;
use edgedb_protocol::query_arg::QueryArgs;
use edgedb_errors::{SHOULD_RETRY};

use crate::client::{StartQuery, execute_query};

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
    pub fn query<R, A>(&mut self, query: &str, arguments: &A)
        -> Result<Vec<R>, Error>
        where A: QueryArgs,
              R: QueryResult,
    {
        execute_query(self, query, arguments)
    }
}
