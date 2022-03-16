pub mod http;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use tokio::fs;
use tokio::sync::RwLock;
use anyhow::Context;

use edgedb_tokio::raw::Pool;

use crate::abi;
use crate::worker;
use crate::tenant::http::ConvertInput as _;

type Database = String;
type WasmName = String;

#[derive(Clone)]
pub struct Tenant(Arc<TenantInner>);

struct TenantInner {
    workers: RwLock<HashSet<worker::Worker>>,
    clients: RwLock<HashMap<Database, Pool>>,
    modules: HashMap<WasmName, wasmtime::Module>,
    engine: wasmtime::Engine,
    linker: wasmtime::Linker<worker::State>,
}

fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    if let Some(c) = chars.next() {
        if !c.is_ascii_alphabetic() {
            return false;
        }
    }
    return chars.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-');
}

impl Tenant {
    pub async fn read_dir(_name: &str, dir: impl AsRef<Path>)
        -> anyhow::Result<Tenant>
    {
        Tenant::_read_dir(dir.as_ref()).await
            .context("failed to read WebAssembly directory")
    }
    async fn _read_dir(dir: &Path) -> anyhow::Result<Tenant>
    {
        let engine = wasmtime::Engine::new(
            wasmtime::Config::new()
            .async_support(true)
        ).context("cannot create engine")?;
        let mut linker = wasmtime::Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, worker::State::wasi)
            .context("error linking WASI")?;
        abi::log_v1::add_to_linker(&mut linker, |s| s)
            .context("error linking edgedb_log_v1")?;
        abi::client_v1::add_to_linker(&mut linker, worker::State::client_v1)
            .context("error linking edgedb_client_v1")?;
        abi::http_server_v1::Handler::add_to_linker(
            &mut linker, worker::State::http_server_v1)
            .context("error linking edgedb_http_server_v1")?;

        let mut dir_iter = fs::read_dir(dir).await?;
        let mut modules = HashMap::new();
        while let Some(item) = dir_iter.next_entry().await? {
            let path = item.path();
            if !matches!(path.extension(), Some(e) if e == "wasm") {
                continue;
            }
            let stem = path.file_stem().and_then(|x| x.to_str());
            let name = if let Some(name) = stem {
                if !is_valid_name(name) {
                    continue;
                }
                name
            } else {
                continue;
            };
            let data = fs::read(&path).await
                .with_context(|| format!("cannot read {path:?}"))?;
            let module = wasmtime::Module::new(&engine, data)
                .context("cannot initialize module")?;
            modules.insert(name.into(), module);
        }
        Ok(Tenant(Arc::new(TenantInner {
            workers: RwLock::new(HashSet::new()),
            clients: RwLock::new(HashMap::new()),
            modules,
            engine,
            linker,
        })))
    }

    pub async fn handle<P>(self, req: P::Input) -> anyhow::Result<P::Output>
        where P: http::Process,
    {
        let cvt = P::read_full(req).await?;
        let mut parts = cvt.uri().path().split('/');
        if parts.next() != Some("") {
            return Ok(P::err_not_found())
        }
        if parts.next() != Some("db") {
            return Ok(P::err_not_found())
        }
        let database = if let Some(db) = parts.next() {
            db
        } else {
            return Ok(P::err_not_found())
        };
        if parts.next() != Some("wasm") {
            return Ok(P::err_not_found())
        }
        let wasm_name = if let Some(name) = parts.next() {
            name
        } else {
            return Ok(P::err_not_found())
        };
        // TODO(tailhook) capture unknown worker error and convert to 404
        let worker = self.get_worker(database, wasm_name).await?;
        worker.handle_http::<P>(cvt).await
    }

    pub fn get_engine(&self) -> &wasmtime::Engine {
        &self.0.engine
    }

    pub fn get_linker(&self) -> &wasmtime::Linker<worker::State> {
        &self.0.linker
    }

    pub async fn get_client(&self, database: &str) -> anyhow::Result<Pool> {
        let clis = &self.0.clients;
        if let Some(pool) = clis.read().await.get(database) {
            return Ok(pool.clone());
        }
        // TODO(tailhook) fix connection credentials
        let mut builder = edgedb_tokio::Builder::uninitialized();
        builder.host_port(Some("localhost"), Some(5656));
        builder.database(database);
        builder.insecure_dev_mode(true);
        let pool = Pool::new(&builder.build()?);
        let mut clis = clis.write().await;
        Ok(clis.entry(database.into())
            .or_insert_with(|| pool)
            .clone())
    }
    pub fn get_module(&self, module: &str) -> Option<&wasmtime::Module> {
        self.0.modules.get(module)
    }
    pub async fn get_worker(&self, database: &str, wasm_name: &str)
        -> anyhow::Result<worker::Worker>
    {
        let name = worker::Name {
            database: database.into(),
            wasm_name: wasm_name.into(),
        };
        let wrks = &self.0.workers;
        if let Some(wrk) = wrks.read().await.get(&name) {
            return Ok(wrk.clone());
        }
        let mut wrks = wrks.write().await;
        if let Some(wrk) = wrks.get(&name) {
            return Ok(wrk.clone());
        }
        let wrk = worker::Worker::new(self, database, wasm_name).await?;
        wrks.insert(wrk.clone());
        Ok(wrk)
    }
}

impl fmt::Debug for Tenant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut dbg = f.debug_struct("Tenant");
        match self.0.workers.try_read() {
            Ok(w) => dbg.field("workers", &w.len()),
            Err(_) => dbg.field("workers", &"--locked--"),
        };
        dbg.finish()
    }
}
