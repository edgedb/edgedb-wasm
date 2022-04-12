use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;


use crate::tenant::TenantInner;

pub struct Module {
    pub path: Arc<PathBuf>,
    pub tenant: Arc<TenantInner>,
    pub modification_time: SystemTime,
    pub wasm: wasmtime::Module,
}
