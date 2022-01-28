use edgedb_sdk::{log, web};

#[web::handler]
fn handler(req: web::Request) -> web::Response {
    todo!();
}

fn main() {
    log::warn!("Hello from Wasm!");
    log::info!("Info from Wasm!");
    log::error!("Error from Wasm!");
}
