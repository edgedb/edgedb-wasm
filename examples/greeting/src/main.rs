use edgedb_sdk::log;

fn main() {
    edgedb_sdk::init();
    log::warn!("Hello from Wasm!");
    log::info!("Info from Wasm!");
    log::error!("Error from Wasm!");
}
