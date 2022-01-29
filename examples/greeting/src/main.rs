use edgedb_sdk::{log, web};

#[edgedb_sdk::init_hook]
fn init_hook() {
    log::warn!("Hello from Init hook!");
}

#[web::handler]
fn handler(req: web::Request) -> web::Response {
    todo!();
}

fn main() {
    log::info!("Info from Wasm!");
    log::error!("Error from Wasm!");
}
