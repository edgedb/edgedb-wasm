pub mod bug;
pub mod http;

pub mod log;

#[cfg(not(feature="host"))]
pub fn init() {
    log::init();
}
