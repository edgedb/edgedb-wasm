pub mod http;

use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::fmt;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use anyhow::Context;
use async_once_cell::OnceCell as Cell;
use edgedb_tokio::raw::Pool;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::sync::{RwLock, Mutex};

use crate::abi;
use crate::worker;
use crate::tenant::http::ConvertInput as _;
use crate::module::Module;

type Database = String;

#[derive(Clone)]
pub struct Tenant(Arc<TenantInner>);

pub struct TenantInner {
    workers: RwLock<HashSet<worker::Worker>>,
    clients: RwLock<HashMap<Database, Pool>>,
    directories: RwLock<HashMap<String, PathBuf>>,
    modules: Mutex<HashMap<Arc<PathBuf>, Arc<Cell<Weak<Module>>>>>,
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
    pub async fn new(_name: &str)
        -> anyhow::Result<Tenant>
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

        Ok(Tenant(Arc::new(TenantInner {
            workers: RwLock::new(HashSet::new()),
            clients: RwLock::new(HashMap::new()),
            modules: Mutex::new(HashMap::new()),
            directories: RwLock::new(HashMap::new()),
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
        if !is_valid_name(wasm_name) {
            return Ok(P::err_not_found())
        }
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

    pub async fn set_directory(&self, database: &str, directory: &Path) {
        self.0.directories.write().await
            .insert(database.into(), directory.into());
        // TODO(tailhook) fix to drain_filter
        let mut wrks = self.0.workers.write().await;
        let old_wrks = mem::replace(&mut *wrks, HashSet::new());
        for wrk in old_wrks {
            if wrk.full_name().database != database ||
                wrk.module().path.parent() == Some(directory)
            {
                wrks.insert(wrk);
            }
        }
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
    pub async fn get_module(&self, database: &str, wasm_name: &str)
        -> anyhow::Result<Arc<Module>>
    {
        let path = self.0.directories.read().await.get(database)
            .with_context(|| format!("no wasm directory is configured \
                                      for the database {:?}", database))?
            .join(format!("{}.wasm", wasm_name));
        self._get_module(path).await
    }

    async fn _get_module(&self, path: impl Into<Arc<PathBuf>>)
        -> anyhow::Result<Arc<Module>>
    {
        let path = path.into();
        loop {
            let entry = {
                let mut modules = self.0.modules.lock().await;
                match modules.entry(path.clone()) {
                    Entry::Vacant(cur) => {
                        Err(cur.insert(Arc::new(Cell::new())).clone())
                    }
                    Entry::Occupied(mut cur) => {
                        if let Some(weak) = cur.get().get() {
                            if let Some(strong) = weak.upgrade() {
                                Ok((cur.get().clone(), strong))
                            } else {
                                Err(cur.insert(Arc::new(Cell::new())).clone())
                            }
                        } else {
                            Err(cur.get().clone())
                        }
                    }
                }
            };

            let (cell, module) = match entry {
                Err(cell) => {
                    let mut result = None;
                    let path = path.clone();
                    let weak = cell.get_or_try_init(async {
                        let mut file = fs::File::open(path.as_path()).await
                            .with_context(|| format!("cannot open {path:?}"))?;
                        let modification_time = file.metadata().await
                            .and_then(|m| m.modified())
                            .with_context(|| format!("cannot stat {path:?}"))?;
                        let mut buf = Vec::with_capacity(65536);
                        file.read_to_end(&mut buf).await
                            .with_context(|| format!("cannot read {path:?}"))?;
                        drop(file);
                        let wasm = wasmtime::Module::new(&self.0.engine, buf)
                            .context("cannot initialize module")?;
                        log::info!("Module {path:?} loaded");
                        let module = Arc::new(Module {
                            path: path.clone(),
                            tenant: self.0.clone(),
                            modification_time,
                            wasm,
                        });
                        result = Some(module.clone());
                        anyhow::Ok(Arc::downgrade(&module))
                    }).await?;
                    if let Some(module) = result {
                        return Ok(module);
                    }
                    if let Some(module) = weak.upgrade() {
                        (cell, module)
                    } else {
                        continue; // weak ref target just dropped, restart
                    }
                }
                Ok(pair) => pair,
            };
            // TODO(tailhook) optimize lookups that are too often
            // TODO(tailhook) delete module immediately if file not found
            let new = fs::metadata(path.as_path()).await?;
            match new.modified() {
                Ok(new) if new > module.modification_time => {
                    let mut lock = self.0.modules.lock().await;
                    let current_cell = lock.get(&path);
                    if current_cell
                        .map(|c| Arc::ptr_eq(c, &cell))
                        .unwrap_or(false)
                    {
                        log::info!("Reloading {path:?}");
                        lock.remove(&path);
                    }
                    continue;
                }
                Ok(_) => return Ok(module),
                Err(e) => {
                    log::warn!("Cannot get file modification time: {e:#}. \
                                Skipping the check and do not refresh");
                }
            }
        }
    }
    pub async fn get_worker(&self, database: &str, wasm_name: &str)
        -> anyhow::Result<worker::Worker>
    {
        let module = self.get_module(database, wasm_name).await?;
        let name = worker::Name {
            database: database.into(),
            wasm_name: wasm_name.into(),
        };
        let wrks = &self.0.workers;
        if let Some(wrk) = wrks.read().await.get(&name) {
            // this checks timestamp
            if Arc::ptr_eq(wrk.module(), &module) {
                return Ok(wrk.clone());
            }
        }
        let mut wrks = wrks.write().await;
        if let Some(wrk) = wrks.get(&name) {
            // This presumably has to be refreshed, but some other thread
            // could have already done that
            if Arc::ptr_eq(wrk.module(), &module) {
                return Ok(wrk.clone());
            }
        }
        let wrk = worker::Worker::new(self, database, wasm_name, module).await?;
        wrks.replace(wrk.clone());
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
