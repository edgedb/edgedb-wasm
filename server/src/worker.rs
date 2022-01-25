use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tokio::fs;

use crate::abi;

#[derive(Clone)]
pub struct Worker(Arc<WorkerInner>);

pub struct State {
    pub tenant: String,
    pub worker: String,
    pub wasi: wasmtime_wasi::WasiCtx,
}

struct WorkerInner {
    path: PathBuf,
    store: wasmtime::Store<State>,
    instance: wasmtime::Instance,
}


impl Worker {
    pub fn name(&self) -> &str {
        &self.0.store.data().worker
    }
    pub async fn new(tenant: String, name: String, path: PathBuf)
        -> anyhow::Result<Worker>
    {
        let data = fs::read(&path).await
            .with_context(|| format!("cannot read {path:?}"))?;
        let engine = wasmtime::Engine::new(
            wasmtime::Config::new()
            .async_support(true)
        ).context("cannot create engine")?;
        let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new()
            .inherit_stdio() // temporary
            .build();
        let state = State {
            tenant,
            worker: name,
            wasi,
        };
        let mut store = wasmtime::Store::new(&engine, state);
        let module = wasmtime::Module::new(&engine, data)
            .context("cannot initialize module")?;
        let mut linker = wasmtime::Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s: &mut State| &mut s.wasi)
            .context("error linking WASI")?;
        abi::log_v1::add_to_linker(&mut linker, |s| s)
            .context("error linking edgedb_log_v1")?;

        let instance = linker.instantiate(&mut store, &module)?;
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
            path,
            store,
            instance,
        })))
    }
}

impl fmt::Debug for Worker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let data = self.0.store.data();
        f.debug_struct("Worker")
            .field("tenant", &data.tenant)
            .field("worker", &data.worker)
            .field("path", &self.0.path.display())
            // TODO(tailhook) add some running info
            .finish()
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("worker::State")
            .field("tenant", &self.tenant)
            .field("worker", &self.worker)
            .finish()
    }
}
