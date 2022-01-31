use once_cell::sync::OnceCell;

pub use edgedb_sdk_macros::web_handler as handler;
pub use http::StatusCode;

pub type Response = http::Response<Vec<u8>>;

pub struct Request {
    pub(crate) inner: http::Request<Vec<u8>>,
}

type WebHandler = fn(Request) -> Response;
pub(crate) static WEB_HANDLER: OnceCell<WebHandler> = OnceCell::new();

pub fn register_handler(f: WebHandler) {
    WEB_HANDLER.set(f).expect("only one handler is expected");
}

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
