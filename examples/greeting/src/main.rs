use edgedb_sdk::{log, web};
use edgedb_sdk::client::{Client, Error, create_client};
use once_cell::sync::Lazy;

static CLIENT: Lazy<Client> = Lazy::new(|| create_client());

#[edgedb_sdk::init_hook]
fn init_hook() {
    log::warn!("Hello from Init hook!");
}

fn wrap_error(f: impl FnOnce() -> Result<web::Response, Error>)
    -> web::Response
{
    match f() {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Error handling request: {:#}", e);
            web::response()
                .status(web::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "text/plain")
                .body(format!("Internal Server Error").into())
                .expect("response is built")
        }
    }
}

#[web::handler]
fn handler(_req: web::Request) -> web::Response {
    wrap_error(|| {
        let counter = CLIENT.query::<i64, _>(
            "SELECT (UPDATE Counter SET { value := .value + 1}).value",
            &(),
        )?.remove(0);
        Ok(web::response()
            .status(web::StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(format!("Visited {counter} times").into())
            .expect("response is built"))
    })
}

fn main() {
    log::info!("Info from Wasm!");
    log::error!("Error from Wasm!");
}
