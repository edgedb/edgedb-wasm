use std::collections::HashMap;
use std::path::Path;

use async_std::fs;
use async_std::stream::StreamExt;
use miette::{IntoDiagnostic, Context};

#[derive(Debug)]
pub struct Worker {
}

#[derive(Debug)]
pub struct Tenant {
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
            match Worker::new(name, &path).await {
                Ok(worker) => {
                    workers.insert(name.into(), worker);
                }
                Err(e) => {
                    log::error!("Error reading {:?}: {:#}", name, e);
                }
            }
        }
        log::info!("Read {} wasm files", workers.len());
        Ok(Tenant { workers })
    }
}

impl Worker {
    async fn new(name: &str, path: impl AsRef<Path>) -> miette::Result<Worker> {
        // TODO(tailhook) read wasm file
        Ok(Worker {
        })
    }
}
