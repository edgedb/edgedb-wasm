mod options;
mod tenant;

use miette::IntoDiagnostic;
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
    builder.filter_module("edgedb_wasm_server", log::LevelFilter::Info);
    builder.init();
}

#[async_std::main]
async fn main() -> miette::Result<()> {
    let options = Options::parse();
    init_logging();
    log::debug!("Options {:#?}", options);

    log::info!("Reading wasm files from {:?}", options.wasm_dir);
    let tentant = Tenant::read_dir(&options.wasm_dir).await?;
    let mut app = tide::new();
    app.at("/").get(hello);
    app.listen(("127.0.0.1", options.port)).await.into_diagnostic()?;
    Ok(())
}
