use std::path::PathBuf;
use std::sync::Arc;


use crate::tenant::TenantInner;

pub struct Module {
    pub path: Arc<PathBuf>,
    pub tenant: Arc<TenantInner>,
    pub wasm: wasmtime::Module,
}
