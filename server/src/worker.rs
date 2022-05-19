use std::default::Default;
use std::fmt;
use std::hash;
use std::sync::Arc;

use anyhow::Context;
use tokio::sync::Mutex;
use wasmtime::Instance;

use crate::abi;
use crate::module::Module;
use crate::tenant::Tenant;
use crate::tenant::http::{self, ConvertInput as _};


#[derive(Clone)]
pub struct Worker(Arc<WorkerInner>);

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Name {
    pub database: String,
    pub wasm_name: String,
}

pub struct State {
    pub name: Arc<Name>,
    pub wasi: wasmtime_wasi::WasiCtx,
    pub http_server_v1: abi::http_server_v1::State,
    pub client_v1: abi::client_v1::State,
}

struct WorkerInner {
    name: Arc<Name>,
    module: Arc<Module>,
    store: Mutex<wasmtime::Store<State>>,
    #[allow(dead_code)] // TODO
    instance: Instance,
    http_server_v1: Option<abi::http_server_v1::Handler<State>>,
}

impl State {
    pub fn wasi(&mut self) -> &mut wasmtime_wasi::WasiCtx {
        &mut self.wasi
    }
    pub fn client_v1(&mut self) -> abi::client_v1::Context {
        self.client_v1.context()
    }
    pub fn http_server_v1(&mut self) -> &mut abi::http_server_v1::State {
        &mut self.http_server_v1
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
    /*
    pub fn name(&self) -> &str {
        &self.0.name.worker
    }
    */
    pub fn full_name(&self) -> &Name {
        &*self.0.name
    }
    pub fn module(&self) -> &Arc<Module> {
        &self.0.module
    }
    pub async fn new(tenant: &Tenant, database: &str, wasm_name: &str,
                     module: Arc<Module>)
        -> anyhow::Result<Worker>
    {
        let name = Arc::new(Name {
            database: database.into(),
            wasm_name: wasm_name.into(),
        });
        let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new()
            .inherit_stdio() // temporary
            .build();
        let cli = tenant.get_client(database).await?;
        let state = State {
            name: name.clone(),
            wasi,
            http_server_v1: Default::default(),
            client_v1: abi::client_v1::State::new(&cli),
        };
        let mut store = wasmtime::Store::new(tenant.get_engine(), state);

        let instance = tenant.get_linker()
            .instantiate(&mut store, &module.wasm)?;
        let http_server_v1 = abi::http_server_v1::Handler::new(
            &mut store, &instance, |s: &mut State| &mut s.http_server_v1)
            .context("error reading edgedb_http_server_v1 handler")?;

        call_init(&mut store, &instance).await?;
        let main = instance.get_typed_func::<(), (), _>(&mut store, "_start")
            .context("get main(_start) function")?;
        main.call_async(&mut store, ()).await.context("call main function")?;

        Ok(Worker(Arc::new(WorkerInner {
            name,
            module,
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
            log::debug!("Response generated, code: {:?}", response.status_code);
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
            .field("database", &name.database)
            .field("wasm_name", &name.wasm_name)
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
        write!(f, "/db/{}/wasm/{}", self.database, self.wasm_name)
    }
}

impl std::borrow::Borrow<Name> for Worker {
    fn borrow(&self) -> &Name {
        &*self.0.name
    }
}

impl hash::Hash for Worker {
    fn hash<H>(&self, state: &mut H)
        where H: hash::Hasher
    {
        self.0.name.hash(state)
    }
}

impl PartialEq for Worker {
    fn eq(&self, other: &Worker) -> bool {
        self.0.name == other.0.name
    }
}

impl Eq for Worker {}

