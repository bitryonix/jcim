# jcim-sdk

`jcim-sdk` is the canonical Rust lifecycle API for JCIM 0.2.

It is service-first:

- it discovers or starts the local `jcimd` service
- it talks to the local gRPC contract
- it exposes typed Rust workflows for project, build, simulator, and physical-card operations
- it re-exports typed ISO/IEC 7816 and GlobalPlatform command helpers from `jcim-core`
- it keeps raw hex and raw transport details at the edges

Typical flow:

```rust
use jcim_core::apdu::CommandApdu;
use jcim_sdk::{CardInstallSource, JcimClient, ProjectRef, SimulationInput};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let client = JcimClient::connect_or_start().await?;
let project = ProjectRef::from_path("examples/satochip/workdir");
let build = client.build_project(&project).await?;
let simulation = client
    .start_simulation(SimulationInput::Project(project))
    .await?;
let response = client
    .transmit_sim_apdu(
        simulation.simulation_ref(),
        &CommandApdu::parse(&hex::decode("00A40400095361746F4368697000")?)?,
    )
    .await?;
assert_eq!(response.sw, 0x9000);
let _install = client
    .install_cap(CardInstallSource::Project(ProjectRef::from_id(build.project.project_id)))
    .await?;
# Ok(())
# }
```

Typed ISO/IEC 7816 and GlobalPlatform flows are also available:

```rust
use jcim_sdk::{Aid, JcimClient, globalplatform};

# async fn admin() -> Result<(), Box<dyn std::error::Error>> {
let client = JcimClient::connect_or_start().await?;
let applet = Aid::from_hex("A000000151000001")?;

let _ = client.iso_select_application_on_card(&applet).await?;
let _ = client
    .gp_get_status_on_card(
        globalplatform::RegistryKind::Applications,
        globalplatform::GetStatusOccurrence::FirstOrAll,
    )
    .await?;
let _ = client
    .gp_set_application_status_on_card(&applet, globalplatform::LockTransition::Lock)
    .await?;
# Ok(())
# }
```

Important:

- GP admin APDUs usually require an authenticated card state or secure channel on real hardware.
- `jcim-sdk` now gives you typed command builders and typed `GET STATUS` parsing, but it does not yet manage full secure-channel cryptography for you.
