//! Task-oriented CLI for the JCIM 0.2 local platform.

#![forbid(unsafe_code)]

mod cli;

#[tokio::main]
async fn main() {
    if let Err(error) = cli::run().await {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
