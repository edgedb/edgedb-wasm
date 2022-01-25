use edgedb_sdk::log;

fn main() {
    log::warn!("Hello from Wasm!");
    log::info!("Info from Wasm!");
    log::error!("Error from Wasm!");
}
