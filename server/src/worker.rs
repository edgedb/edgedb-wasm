use std::default::Default;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tokio::fs;
use tokio::sync::Mutex;
use wasmtime::Instance;

use crate::abi;
use crate::tenant::Common;
use crate::tenant::http::{self, ConvertInput as _};


#[derive(Clone)]
pub struct Worker(Arc<WorkerInner>);

#[derive(Debug)]
pub struct Name {
    pub tenant: String,
    pub worker: String,
    pub path: PathBuf,
}

pub struct State {
    pub name: Arc<Name>,
    pub wasi: wasmtime_wasi::WasiCtx,
    pub http_server_v1: abi::http_server_v1::State,
    pub client_v1: abi::client_v1::State,
}

struct WorkerInner {
    name: Arc<Name>,
    store: Mutex<wasmtime::Store<State>>,
    #[allow(dead_code)] // TODO
    instance: Instance,
    http_server_v1: Option<abi::http_server_v1::Handler<State>>,
}

impl State {
    fn client_v1(&mut self) -> abi::client_v1::Context {
        self.client_v1.context()
    }
}

async fn call_init(store: &mut wasmtime::Store<State>, instance: &Instance)
    -> anyhow::Result<()>
{
    let mut pre_init = None;
    let mut post_init = None;
    let init_funcs = instance.exports(&mut *store).filter_map(|e| {
        match e.name() {
            "_edgedb_sdk_pre_init" => {
                pre_init = e.into_func();
                None
            }
            "_edgedb_sdk_post_init" => {
                post_init = e.into_func();
                None
            }
            name if name.starts_with("_edgedb_sdk_init_") => {
                e.into_func()
            }
            _ => None,
        }
    }).collect::<Vec<_>>();
    if let Some(pre_init) = pre_init {
        pre_init.typed(&mut *store)
            .with_context(|| format!("{:?} has wrong type", pre_init))?
            .call_async(&mut *store, ()).await
            .with_context(|| format!("error calling {:?}", pre_init))?;
    }
    for init_func in init_funcs {
        init_func.typed(&mut *store)
            .with_context(|| format!("{:?} has wrong type", init_func))?
            .call_async(&mut *store, ()).await
            .with_context(|| format!("error calling {:?}", init_func))?;
    }
    if let Some(post_init) = post_init {
        post_init.typed(&mut *store)
            .with_context(|| format!("{:?} has wrong type", post_init))?
            .call_async(&mut *store, ()).await
            .with_context(|| format!("error calling {:?}", post_init))?;
    }
    Ok(())
}

impl Worker {
    pub fn name(&self) -> &str {
        &self.0.name.worker
    }
    pub fn full_name(&self) -> &Name {
        &*self.0.name
    }
    pub async fn new(tenant: String, name: String, path: PathBuf,
                     common: Common)
        -> anyhow::Result<Worker>
    {
        let data = fs::read(&path).await
            .with_context(|| format!("cannot read {path:?}"))?;
        let name = Arc::new(Name {
            tenant,
            worker: name,
            path,
        });
        let engine = wasmtime::Engine::new(
            wasmtime::Config::new()
            .async_support(true)
        ).context("cannot create engine")?;
        let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new()
            .inherit_stdio() // temporary
            .build();
        let state = State {
            name: name.clone(),
            wasi,
            http_server_v1: Default::default(),
            client_v1: abi::client_v1::State::new(&common.client),
        };
        let mut store = wasmtime::Store::new(&engine, state);
        let module = wasmtime::Module::new(&engine, data)
            .context("cannot initialize module")?;
        let mut linker = wasmtime::Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s: &mut State| &mut s.wasi)
            .context("error linking WASI")?;
        abi::log_v1::add_to_linker(&mut linker, |s| s)
            .context("error linking edgedb_log_v1")?;
        abi::client_v1::add_to_linker(&mut linker, State::client_v1)
            .context("error linking edgedb_client_v1")?;
        abi::http_server_v1::Handler::add_to_linker(
            &mut linker, |s: &mut State| &mut s.http_server_v1)
            .context("error linking edgedb_http_server_v1")?;

        let instance = linker.instantiate(&mut store, &module)?;
        let http_server_v1 = abi::http_server_v1::Handler::new(
            &mut store, &instance, |s: &mut State| &mut s.http_server_v1)
            .context("error reading edgedb_http_server_v1 handler")?;

        call_init(&mut store, &instance).await?;
        let main = instance.get_typed_func::<(), (), _>(&mut store, "_start")
            .context("get main(_start) function")?;
        main.call_async(&mut store, ()).await.context("call main function")?;

        Ok(Worker(Arc::new(WorkerInner {
            name,
            store: Mutex::new(store),
            instance,
            http_server_v1: Some(http_server_v1),
        })))
    }
    pub async fn handle_http<P: http::Process>(&self, req: P::ConvertInput)
        -> anyhow::Result<P::Output>
    {
        if let Some(api) = &self.0.http_server_v1 {
            let response;
            // TODO(tailhook) on poison restart worker
            let mut store = self.0.store.lock().await;
            match api.handle_request(&mut *store, req.as_v1()).await {
                Ok(resp) => response = resp,
                Err(e) => {
                    log::error!("Worker {} failed to handle request: {:#}. \
                                 Request: {:?}",
                                self.full_name(), e, req);
                    return Ok(P::err_internal_server_error())
                }
            };
            match http::FromWasm::from_wasm(response) {
                Ok(resp) => Ok(resp),
                Err(e) => {
                    log::error!("Worker {} returned invalid response: {:#}. \
                                 Request: {:?}",
                                self.full_name(), e, req);
                    Ok(P::err_internal_server_error())
                }
            }
        } else {
            Ok(P::err_not_found())
        }
    }
}

impl fmt::Debug for Worker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = &self.0.name;
        f.debug_struct("Worker")
            .field("tenant", &name.tenant)
            .field("worker", &name.worker)
            .field("path", &name.path.display())
            // TODO(tailhook) add some running info
            .finish()
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("worker::State")
            .field("worker", &self.name)
            .finish()
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.tenant, self.worker)
    }
}
