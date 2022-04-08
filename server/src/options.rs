use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
pub struct Options {
    /// Port for the server to listen to
    #[clap(short='P', long, conflicts_with="unix-socket",
           default_value="5657")]
    pub port: u16,

    /// Port for the server to listen to
    #[clap(long, conflicts_with_all=&["port", "wasm-dir"])]
    pub unix_socket: Option<PathBuf>,

    /// Directory with wasm files (for single tenant)
    #[clap(long, conflicts_with="unix-socket")]
    pub wasm_dir: Option<PathBuf>,
}
