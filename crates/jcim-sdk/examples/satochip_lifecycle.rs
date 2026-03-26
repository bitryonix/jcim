//! Source-backed Satochip lifecycle demo through the JCIM Rust SDK.

#![forbid(unsafe_code)]
#![allow(clippy::missing_docs_in_private_items)]

use std::path::PathBuf;

use jcim_core::apdu::CommandApdu;
use jcim_sdk::{CardInstallSource, JcimClient, ProjectRef, ReaderRef, SimulationInput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = JcimClient::connect_or_start().await?;
    let project = ProjectRef::from_path(satochip_project_root());

    let build = client.build_project(&project).await?;
    println!("Built {} artifact(s).", build.artifacts.len());

    let simulation = client
        .start_simulation(SimulationInput::Project(project.clone()))
        .await?;
    println!("Started simulation {}", simulation.simulation_id);

    let select = CommandApdu::parse(&hex::decode("00A40400095361746F4368697000")?)?;
    let select_response = client
        .transmit_sim_apdu(simulation.simulation_ref(), &select)
        .await?;
    println!("Simulator SELECT status: {:04X}", select_response.sw);

    let status = CommandApdu::parse(&hex::decode("B03C000000")?)?;
    let status_response = client
        .transmit_sim_apdu(simulation.simulation_ref(), &status)
        .await?;
    println!(
        "Simulator status APDU: {}",
        hex::encode_upper(status_response.to_bytes())
    );

    let _atr = client.reset_simulation(simulation.simulation_ref()).await?;
    let _stopped = client.stop_simulation(simulation.simulation_ref()).await?;

    if let Ok(reader_name) = std::env::var("JCIM_EXAMPLE_CARD_READER") {
        let install = client
            .install_cap_on(
                CardInstallSource::Project(project),
                ReaderRef::named(reader_name.clone()),
            )
            .await?;
        println!(
            "Installed {} ({}) on {}",
            install.package_name, install.package_aid, reader_name
        );
        let response = client
            .transmit_card_apdu_on(&select, ReaderRef::named(reader_name.clone()))
            .await?;
        println!("Card SELECT status: {:04X}", response.sw);
        let _atr = client.reset_card_on(ReaderRef::named(reader_name)).await?;
    } else {
        println!(
            "Skipping physical-card install. Set JCIM_EXAMPLE_CARD_READER to run the real-card leg."
        );
    }

    Ok(())
}

fn satochip_project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/satochip/workdir")
}
