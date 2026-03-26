//! Binary entry point for the JCIM 0.2 local service.
#![allow(clippy::missing_docs_in_private_items)]
#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use jcim_app::JcimApp;

#[derive(Debug, Parser)]
#[command(name = "jcimd")]
#[command(about = "JCIM 0.2 local gRPC service")]
struct Args {
    /// Override the managed Unix-domain socket path used by the local service.
    #[arg(long)]
    socket_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let app = JcimApp::load()?;
    let socket_path = args
        .socket_path
        .unwrap_or_else(|| app.managed_paths().service_socket_path.clone());
    jcimd::serve_local_service(app, &socket_path).await?;
    Ok(())
}
