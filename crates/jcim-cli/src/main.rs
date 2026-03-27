//! Task-oriented CLI for the JCIM 0.3 local platform.

#![forbid(unsafe_code)]

mod cli;

#[tokio::main]
async fn main() {
    if let Err(error) = cli::run().await {
        if error.json_mode() {
            eprintln!("{}", error.json_output());
        } else {
            eprintln!("Error: {error}");
        }
        std::process::exit(1);
    }
}
