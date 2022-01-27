use std::default::Default;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tokio::fs;
use tokio::sync::Mutex;

use crate::abi;

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
}

struct WorkerInner {
    name: Arc<Name>,
    store: Mutex<wasmtime::Store<State>>,
    instance: wasmtime::Instance,
    http_server_v1: Option<abi::http_server_v1::Handler<State>>,
}

impl Worker {
    pub fn name(&self) -> &str {
        &self.0.name.worker
    }
    pub fn full_name(&self) -> &Name {
        &*self.0.name
    }
    pub async fn new(tenant: String, name: String, path: PathBuf)
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
        };
        let mut store = wasmtime::Store::new(&engine, state);
        let module = wasmtime::Module::new(&engine, data)
            .context("cannot initialize module")?;
        let mut linker = wasmtime::Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s: &mut State| &mut s.wasi)
            .context("error linking WASI")?;
        abi::log_v1::add_to_linker(&mut linker, |s| s)
            .context("error linking edgedb_log_v1")?;
        abi::http_server_v1::Handler::add_to_linker(
            &mut linker, |s: &mut State| &mut s.http_server_v1)
            .context("error linking edgedb_http_server_v1")?;

        let instance = linker.instantiate(&mut store, &module)?;
        let http_server_v1 = abi::http_server_v1::Handler::new(
            &mut store, &instance, |s: &mut State| &mut s.http_server_v1)
            .context("error reading edgedb_http_server_v1 handler")?;

        let init_func = instance.get_typed_func::<(), (), _>(
            &mut store,
            "_edgedb_sdk_init"
        );
        match init_func {
            Ok(init_func) => {
                init_func.call_async(&mut store, ()).await
                    .context("SDK init function")?;
            }
            Err(e) => {
                // TODO(tailhook) do not crash if not found
                log::warn!("Cannot initialize EdgeDB SDK: {:#}", e);
            }
        }
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
    pub async fn handle_http(&self, mut req: hyper::Request<hyper::Body>)
        -> hyper::Result<hyper::Response<hyper::Body>>
    {
        if let Some(api) = &self.0.http_server_v1 {
            let api_req =
                abi::http_server_v1::ConvertRequest::read_full(&mut req).await?;
            let response;
            // TODO(tailhook) on poison restart worker
            let mut store = self.0.store.lock().await;
            match api.handle_request(&mut *store, api_req.as_request()).await {
                Ok(resp) => response = resp,
                Err(e) => {
                    log::error!("Worker {} failed to handle request: {:#}. \
                                 Request: {:?}",
                                self.full_name(), e, req);
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        // TODO(tailhook) only in debug mode
                        .body("Wasm failed to handle request".into())
                        .expect("can compose static response"))
                }
            };
            match response.try_into() {
                Ok(resp) => Ok(resp),
                Err(e) => {
                    log::error!("Worker {} returned invalid response: {:#}. \
                                 Request: {:?}",
                                self.full_name(), e, req);
                    Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        // TODO(tailhook) only in debug mode
                        .body("Wasm returned invalid response".into())
                        .expect("can compose static response"))
                }
            }
        } else {
            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                // TODO(tailhook) only in debug mode
                .body(format!("Worker {} does not support HTTP",
                              self.full_name()).into())
                .expect("can compose static response"))
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
