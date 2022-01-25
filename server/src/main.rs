mod abi;
mod options;
mod tenant;
mod worker;

use anyhow::Context;
use clap::Parser;

use options::Options;
use tenant::Tenant;

async fn hello(_req: tide::Request<()>) -> tide::Result {
    Ok(format!("Hello world!").into())
}

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
    let mut app = tide::new();
    app.at("/").get(hello);
    for (name, handler)  in tenant.handlers() {
        let mut path = format!("/wasm/edgedb/{}", name);
        log::info!("registering path {:?}", path);
        app.at(&path).all(handler.clone());
        path.push_str("/*");
        app.at(&path).all(handler);
    }
    app.listen(("127.0.0.1", options.port)).await.context("error listening")?;
    Ok(())
}
