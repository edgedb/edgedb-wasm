use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
pub struct Options {
    /// Port for the server to listen to
    #[clap(short='P', long, default_value="5657")]
    pub port: u16,

    /// Directory with wasm files (for single tenant)
    #[clap(long, default_value="./wasm")]
    pub wasm_dir: PathBuf,
}
