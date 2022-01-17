use std::collections::HashMap;

use async_std::fs;
use async_std::path::{Path, PathBuf};
use async_std::stream::StreamExt;
use miette::{IntoDiagnostic, Context};

#[derive(Debug)]
pub struct Worker {
    name: String,
    path: PathBuf,
    store: wasmer::Store,
    instance: wasmer::Instance,
}

#[derive(Debug)]
pub struct Tenant {
    // TODO(tailhook) maybe set of workers?
    workers: HashMap<String, Worker>,
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
    pub async fn read_dir(dir: impl AsRef<Path>) -> miette::Result<Tenant> {
        Tenant::_read_dir(dir.as_ref()).await
            .wrap_err("failed to read WebAssembly directory")
    }
    async fn _read_dir(dir: &Path) -> miette::Result<Tenant> {
        let mut workers = HashMap::new();
        let mut dir_iter = fs::read_dir(dir).await.into_diagnostic()?;
        let mut tasks = Vec::new();
        while let Some(entry) = dir_iter.next().await {
            let item = entry.into_diagnostic()?;
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
                    workers.insert(worker.name.clone(), worker);
                }
                Err(e) => {
                    log::error!("Error reading {:?}: {:#}", path, e);
                }
            }
        }
        log::info!("Read {} wasm files", workers.len());
        Ok(Tenant { workers })
    }
}

impl Worker {
    async fn new(name: String, path: PathBuf) -> miette::Result<Worker> {
        let data = fs::read(&path).await.into_diagnostic()?;
        let store = wasmer::Store::default();
        let module = wasmer::Module::new(&store, data).into_diagnostic()?;
        let mut wasi_env = wasmer_wasi::WasiState::new(&name).finalize()
            .into_diagnostic().wrap_err("failed to finalize wasi state")?;
        let imports = wasi_env.import_object(&module)
            .into_diagnostic().wrap_err("failed to resolve imports")?;
        let instance = wasmer::Instance::new(&module, &imports)
            .into_diagnostic().wrap_err("instance init failed")?;
        let main = instance.exports.get_function("_start")
            .into_diagnostic().wrap_err("get main(_start) function")?;
        main.call(&[]).into_diagnostic().wrap_err("call main function")?;
        Ok(Worker {
            name,
            path,
            store,
            instance,
        })
    }
}
