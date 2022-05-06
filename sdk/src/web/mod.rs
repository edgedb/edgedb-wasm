//! Web server handling API
//!
//! This module contains facilities needed to process HTTP requests.
//!
//! # Register Web Handler
//!
//! To register web handler it's usually enoght to use the attribute:
//!
//! ```rust,no_run
//! use edgedb_sdk::web;
//! #[web::handler]
//! fn web_handler(req: web::Request) -> web::Response {
//!     web::response()
//!         .status(web::StatusCode::OK)
//!         .header("Content-Type", "text/html")
//!         .body("Hello <b>World</b>".into())
//!         .expect("response is built")
//! }
//! ```
//!
//! # Programmatically Register Web Handler
//!
//! It's sometimes useful to do that programmatically. This is usually done in
//! [`init_hook`](macro@crate::init_hook) and [`register_handler()`]:
//!
//! ```rust,no_run
//! use edgedb_sdk::{init_hook, web};
//!
//! #[init_hook]
//! fn init() {
//!     web::register_handler(web_handler);
//! }
//!
//! fn web_handler(req: web::Request) -> web::Response {
//!     todo!();
//! }
//! ```
use once_cell::sync::OnceCell;

pub use edgedb_sdk_macros::web_handler as handler;
pub use http::StatusCode;

/// Re-exported type from [`http`](http::Response) crate
pub type Response = http::Response<Vec<u8>>;

/// Web Request
///
/// Currently it dereferences to [`http::Request`] so see its documentation
/// for more info.
#[derive(Debug)]
pub struct Request {
    pub(crate) inner: http::Request<Vec<u8>>,
}

type WebHandler = fn(Request) -> Response;
pub(crate) static WEB_HANDLER: OnceCell<WebHandler> = OnceCell::new();

/// Register a function as a web handler
///
/// # Panics
///
/// Panics if called more than once (including implicitly by [`handler`]
/// macro).
pub fn register_handler(f: WebHandler) {
    WEB_HANDLER.set(f).expect("only one handler is expected");
}

/// Create a response builder
///
/// See [`http`](`http::response::Builder`) crate documentation for more info.
pub fn response() -> http::response::Builder {
    http::Response::builder()
}

impl AsRef<http::Request<Vec<u8>>> for Request {
    fn as_ref(&self) -> &http::Request<Vec<u8>> {
        &self.inner
    }
}

impl std::ops::Deref for Request {
    type Target = http::Request<Vec<u8>>;
    fn deref(&self) -> &http::Request<Vec<u8>> {
        &self.inner
    }
}

impl Into<http::Request<Vec<u8>>> for Request {
    fn into(self) -> http::Request<Vec<u8>> {
        self.inner
    }
}
