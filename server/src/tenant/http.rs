use std::fmt;

use hyper::Uri;

use crate::abi::http_server_v1 as v1;


#[async_trait::async_trait]
pub trait Process {
    type Input;
    type ConvertInput: ConvertInput;
    type Output: FromWasm;
    async fn read_full(input: Self::Input)
        -> anyhow::Result<Self::ConvertInput>;
    fn err_not_found() -> Self::Output;
    fn err_internal_server_error() -> Self::Output;
}

pub trait ConvertInput: fmt::Debug {
    fn uri(&self) -> &Uri;
    fn as_v1(&self) -> v1::Request<'_>;
}

pub trait FromWasm: Sized {
    fn from_wasm(wasm: v1::Response) -> anyhow::Result<Self>;
}
