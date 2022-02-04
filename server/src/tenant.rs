use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::fs;
use anyhow::Context;

use edgedb_tokio::raw::Pool;

use crate::worker;

#[derive(Debug, Clone)]
pub struct Tenant(Arc<TenantInner>);

#[derive(Debug)]
struct TenantInner {
    // TODO(tailhook) maybe set of workers?
    workers: HashMap<String, worker::Worker>,
    common: Common,
}

#[derive(Clone, Debug)]
pub struct Common {
    pub client: Pool,
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
        let mut builder = edgedb_tokio::Builder::uninitialized();
        // TODO(tailhook) temporary
        builder.host_port(Some("localhost"), Some(5656));
        let tenant = Common {
            client: Pool::new(builder),
        };
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
                tenant.clone(),
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
        Ok(Tenant(Arc::new(TenantInner {
            workers,
            common: tenant,
        })))
    }

    pub async fn handle(self, req: hyper::Request<hyper::Body>)
        -> hyper::Result<hyper::Response<hyper::Body>>
    {
        if let Some(suffix) = req.uri().path().strip_prefix("/db/default/") {
            let name_end = suffix.find('/').unwrap_or(suffix.len());
            let wasm_name = &suffix[..name_end];
            if let Some(worker) = self.0.workers.get(wasm_name) {
                worker.handle_http(req).await
            } else {
                // TODO(tailhook) only in debug mode
                Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::NOT_FOUND)
                    .body(format!("No wasm named {wasm_name} found").into())
                    .expect("can compose static response"))
            }
        } else {
            // TODO(tailhook) only in debug mode
            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(b"Try /db/default/<wasm-file-name>/"[..].into())
                .expect("can compose static response"))
        }
    }
}
