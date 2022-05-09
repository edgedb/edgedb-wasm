//! EdgeDB WebAssembly SDK
//!
//! # Example
//!
//! The primary use of this SDK is to create a web handler, and to make some
//! database queries inside that handler. Somewhat minimal example:
//!
//! ```rust,no_run
//! use edgedb_sdk::web;
//! use edgedb_sdk::client::{Client, create_client};
//! use once_cell::sync::Lazy;
//!
//! static CLIENT: Lazy<Client> = Lazy::new(|| create_client());
//!
//! #[web::handler]
//! fn handler(_req: web::Request) -> web::Response {
//!     let query_result = CLIENT.query_required_single::<i64, _>(
//!         "SELECT 7*8",
//!         &(),
//!     );
//!     match query_result {
//!         Ok(value) => {
//!             web::response()
//!                 .status(web::StatusCode::OK)
//!                 .header("Content-Type", "text/html")
//!                 .body(format!("7 times 8 is {value}").into())
//!                 .expect("response is built")
//!         }
//!         Err(e) => {
//!             log::error!("Error handling request: {:#}", e);
//!             web::response()
//!                 .status(web::StatusCode::INTERNAL_SERVER_ERROR)
//!                 .header("Content-Type", "text/plain")
//!                 .body(format!("Internal Server Error").into())
//!                 .expect("response is built")
//!         }
//!     }
//! }
//! ```
#![warn(missing_debug_implementations, missing_docs)]

mod http_server;
mod bug;
mod bindgen;

#[cfg(feature="client")]
pub mod client;
pub mod web;
pub mod log;

pub use edgedb_sdk_macros::init_hook;

#[cfg(not(feature="host"))]
#[export_name = "_edgedb_sdk_pre_init"]
extern "C" fn init() {
    log::init();
}
