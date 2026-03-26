//! Source-backed Satochip wallet demo through the JCIM Rust SDK.
//!
//! By default this example builds the vendored Satochip project and runs the wallet flow against a
//! fresh virtual simulation. Pass `--reader <name>` to install the built CAP onto a physical card
//! and run the same secure-channel wallet/signing flow there instead.

#![forbid(unsafe_code)]
#![allow(clippy::missing_docs_in_private_items)]

#[path = "support/satochip.rs"]
mod satochip_support;

use std::path::PathBuf;

use jcim_sdk::{
    CardConnectionTarget, CardInstallSource, JcimClient, ProjectRef, ReaderRef, SimulationInput,
};

struct ExampleArgs {
    project_path: PathBuf,
    reader_name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = parse_args()?;
    let client = JcimClient::connect_or_start().await?;
    let project = ProjectRef::from_path(args.project_path.clone());

    let build = client.build_project(&project).await?;
    println!(
        "Built {} artifact(s); primary CAP: {}",
        build.artifacts.len(),
        build.artifacts[0].path.display()
    );

    let connection = if let Some(reader_name) = args.reader_name.clone() {
        let install = client
            .install_cap_on(
                CardInstallSource::Project(project.clone()),
                ReaderRef::named(reader_name.clone()),
            )
            .await?;
        println!(
            "Installed {} ({}) on {}",
            install.package_name, install.package_aid, reader_name
        );
        client
            .open_card_connection(CardConnectionTarget::Reader(ReaderRef::named(reader_name)))
            .await?
    } else {
        let connection = client
            .open_card_connection(CardConnectionTarget::StartSimulation(
                SimulationInput::Project(project.clone()),
            ))
            .await?;
        println!(
            "Started virtual Satochip target: {:?}",
            connection.locator()
        );
        connection
    };

    let flow_result = satochip_support::run_wallet_demo(&connection).await;
    let close_result = connection.close().await;

    let flow = flow_result?;
    close_result?;

    println!(
        "Initial status: protocol {}.{} applet {}.{} secure_channel={} setup={} seeded={}",
        flow.initial_status.protocol_version.0,
        flow.initial_status.protocol_version.1,
        flow.initial_status.applet_version.0,
        flow.initial_status.applet_version.1,
        flow.initial_status.needs_secure_channel,
        flow.initial_status.setup_done,
        flow.initial_status.seeded
    );
    println!(
        "Post-setup status: setup={} seeded={} pin0_tries={}",
        flow.post_setup_status.setup_done,
        flow.post_setup_status.seeded,
        flow.post_setup_status.pin0_tries_remaining
    );
    println!(
        "Post-seed status: setup={} seeded={} pin0_tries={}",
        flow.post_seed_status.setup_done,
        flow.post_seed_status.seeded,
        flow.post_seed_status.pin0_tries_remaining
    );
    println!("Authentikey: {}", flow.authentikey_hex);
    println!("Derived pubkey: {}", flow.derived_pubkey_hex);
    println!("Derived chain code: {}", flow.chain_code_hex);
    println!("Demo transaction bytes: {}", flow.transaction_hex);
    println!("Demo transaction hash: {}", flow.transaction_hash_hex);
    println!("Transaction signature: {}", flow.signature_hex);
    println!(
        "Wallet created with primary PIN {:?}; the demo transaction signature verified locally.",
        String::from_utf8_lossy(b"123456")
    );

    Ok(())
}

fn parse_args() -> Result<ExampleArgs, Box<dyn std::error::Error + Send + Sync>> {
    let mut project_path = satochip_support::satochip_project_root();
    let mut reader_name = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--project" => {
                let Some(path) = args.next() else {
                    return Err("--project requires a path argument".into());
                };
                project_path = PathBuf::from(path);
            }
            "--reader" => {
                let Some(reader) = args.next() else {
                    return Err("--reader requires a reader name".into());
                };
                reader_name = Some(reader);
            }
            other => {
                return Err(format!("unrecognized argument: {other}").into());
            }
        }
    }

    Ok(ExampleArgs {
        project_path,
        reader_name,
    })
}

fn print_usage() {
    println!(
        "Usage: cargo run -p jcim-sdk --example satochip_wallet -- [--project <path>] [--reader <reader name>]"
    );
    println!();
    println!("Without --reader the example starts a fresh virtual Satochip simulation.");
    println!(
        "With --reader it installs the built CAP onto that reader and runs the same flow there."
    );
}
