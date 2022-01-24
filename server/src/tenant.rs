use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fmt;

use tokio::fs;
use anyhow::Context;

#[derive(Clone)]
pub struct Worker(Arc<WorkerInner>);

struct WorkerInner {
    name: String,
    path: PathBuf,
    store: wasmtime::Store<wasmtime_wasi::WasiCtx>,
    instance: wasmtime::Instance,
}

#[derive(Debug)]
pub struct Tenant {
    // TODO(tailhook) maybe set of workers?
    workers: HashMap<String, Worker>,
}

#[derive(Debug, Clone)]
pub struct Handler {
    worker: Worker,
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
    pub async fn read_dir(dir: impl AsRef<Path>) -> anyhow::Result<Tenant> {
        Tenant::_read_dir(dir.as_ref()).await
            .context("failed to read WebAssembly directory")
    }
    async fn _read_dir(dir: &Path) -> anyhow::Result<Tenant> {
        let mut workers = HashMap::new();
        let mut dir_iter = fs::read_dir(dir).await?;
        let mut tasks = Vec::new();
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
            tasks.push((Worker::new(name.into(), path.clone()), path));
        }
        for (task, path) in tasks {
            match task.await {
                Ok(worker) => {
                    workers.insert(worker.0.name.clone(), worker);
                }
                Err(e) => {
                    log::error!("Error reading {:?}: {:#}", path, e);
                }
            }
        }
        log::info!("Read {} wasm files", workers.len());
        Ok(Tenant { workers })
    }

    pub fn handlers(&self) -> impl Iterator<Item=(&str, Handler)> {
        self.workers.iter().map(|(name, worker)| {
            (&name[..], Handler::new(worker))
        })
    }
}

impl Worker {
    async fn new(name: String, path: PathBuf) -> anyhow::Result<Worker> {
        let data = fs::read(&path).await
            .with_context(|| format!("cannot read {path:?}"))?;
        let engine = wasmtime::Engine::new(
            wasmtime::Config::new()
            .async_support(true)
        ).context("cannot create engine")?;
        let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new()
            .inherit_stdio() // temporary
            .build();
        let mut store = wasmtime::Store::new(&engine, wasi);
        let module = wasmtime::Module::new(&engine, data)
            .context("cannot initialize module")?;
        let mut linker = wasmtime::Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)
            .context("error linking WASI")?;

        let instance = linker.instantiate(&mut store, &module)?;
        let main = instance.get_typed_func::<(), (), _>(&mut store, "_start")
            .context("get main(_start) function")?;
        main.call_async(&mut store, ()).await.context("call main function")?;

        Ok(Worker(Arc::new(WorkerInner {
            name,
            path,
            store,
            instance,
        })))
    }
}

impl Handler {
    fn new(worker: &Worker) -> Handler {
        Handler {
            worker: worker.clone(),
        }
    }
}

#[async_trait::async_trait]
impl tide::Endpoint<()> for Handler {
    async fn call(&self, req: tide::Request<()>) -> tide::Result {
        todo!();
    }
}

impl fmt::Debug for Worker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Worker")
            // TODO(tailhook) add tenant name?
            .field("name", &self.0.name)
            .field("path", &self.0.path.display())
            // TODO(tailhook) add some running info
            .finish()
    }
}
