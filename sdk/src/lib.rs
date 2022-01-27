pub mod bug;
pub mod http_server;

pub mod log;

#[cfg(not(feature="host"))]
#[export_name = "_edgedb_sdk_init"]
extern "C" fn init() {
    log::init();
}
