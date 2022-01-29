pub mod bug;
pub mod http_server;
pub mod web;
pub mod log;

pub use edgedb_sdk_macros::init_hook;

#[cfg(not(feature="host"))]
#[export_name = "_edgedb_sdk_pre_init"]
extern "C" fn init() {
    log::init();
}
