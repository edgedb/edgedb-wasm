use std::collections::HashMap;
use std::path::Path;

use tokio::fs;
use anyhow::Context;

use crate::worker;

#[derive(Debug)]
pub struct Tenant {
    // TODO(tailhook) maybe set of workers?
    workers: HashMap<String, worker::Worker>,
}

#[derive(Debug, Clone)]
pub struct Handler {
    worker: worker::Worker,
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
    pub async fn read_dir(name: &str, dir: impl AsRef<Path>)
        -> anyhow::Result<Tenant>
    {
        Tenant::_read_dir(name, dir.as_ref()).await
            .context("failed to read WebAssembly directory")
    }
    async fn _read_dir(tenant_name: &str, dir: &Path) -> anyhow::Result<Tenant>
    {
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
            let worker = worker::Worker::new(
                tenant_name.to_string(),
                name.into(),
                path.clone(),
            );
            let task = tokio::spawn(worker);
            tasks.push((task, path));
        }
        for (task, path) in tasks {
            match task.await {
                Ok(Ok(worker)) => {
                    workers.insert(worker.name().to_string(), worker);
                }
                Ok(Err(e)) => {
                    log::error!("Error reading {:?}: {:#}", path, e);
                }
                Err(e) => {
                    log::error!("Error waiting worker {:?}: {:#}", path, e);
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


impl Handler {
    fn new(worker: &worker::Worker) -> Handler {
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
