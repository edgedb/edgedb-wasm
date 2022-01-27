mod abi;
mod options;
mod tenant;
mod worker;

use std::convert::Infallible;
use std::net::SocketAddr;

use anyhow::Context;
use clap::Parser;
use hyper::{Server};
use hyper::service::{make_service_fn, service_fn};

use options::Options;
use tenant::Tenant;


pub fn init_logging() {
    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn")
    );
    builder.filter_module("tide", log::LevelFilter::Info);
    builder.filter_module("wasm", log::LevelFilter::Info);
    builder.filter_module("edgedb_wasm_server", log::LevelFilter::Info);
    builder.init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let options = Options::parse();
    init_logging();
    log::debug!("Options {:#?}", options);

    log::info!("Reading wasm files from {:?}", options.wasm_dir);
    let tenant = Tenant::read_dir("default", &options.wasm_dir).await?;

    let addr = SocketAddr::from(([127, 0, 0, 1], options.port));

    let make_svc = make_service_fn(|_conn| {
        let tenant = tenant.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                tenant.clone().handle(req)
            }))
        }
    });

    Server::bind(&addr).serve(make_svc).await.context("error serving HTTP")?;

    Ok(())
}
