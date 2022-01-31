use edgedb_sdk::{log, web};

#[edgedb_sdk::init_hook]
fn init_hook() {
    log::warn!("Hello from Init hook!");
}

#[web::handler]
fn handler(req: web::Request) -> web::Response {
    web::response()
        .status(web::StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(b"Hello from <b>wasm</b>!".to_vec())
        .expect("response is built")
}

fn main() {
    log::info!("Info from Wasm!");
    log::error!("Error from Wasm!");
}
