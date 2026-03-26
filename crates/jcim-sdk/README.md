# jcim-sdk

`jcim-sdk` is the canonical Rust lifecycle API for JCIM 0.2.

It is service-first:

- it discovers or starts the local `jcimd` service
- it talks to the local gRPC contract
- it exposes typed Rust workflows for project, build, simulator, and physical-card operations
- it exposes one unified APDU connection surface for real and virtual cards
- it re-exports typed ISO/IEC 7816 and GlobalPlatform command helpers from `jcim-core`
- it keeps raw hex and raw transport details at the edges

On macOS and Linux, the maintained simulator/build/helper path uses the repository-bundled Temurin
11 runtime. No Docker, container command, host Java install, or `JAVA_HOME` is required for the
managed simulator path.

Typical flow:

```rust
use jcim_sdk::{CardConnectionTarget, CommandApdu, JcimClient, ProjectRef, SimulationInput};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let client = JcimClient::connect_or_start().await?;
let connection = client
    .open_card_connection(CardConnectionTarget::StartSimulation(
        SimulationInput::Project(ProjectRef::from_path("examples/satochip/workdir")),
    ))
    .await?;
let select = CommandApdu::parse(&[
    0x00, 0xA4, 0x04, 0x00, 0x09, 0x53, 0x61, 0x74, 0x6F, 0x43, 0x68, 0x69, 0x70, 0x00,
])?;
let response = connection.transmit(&select).await?;
assert_eq!(response.sw, 0x9000);
connection.close().await?;
# Ok(())
# }
```

Real-card and virtual-card APDU traffic share the same connection object:

```rust
use jcim_sdk::{CardConnectionTarget, JcimClient, ReaderRef};

# async fn reader_demo() -> Result<(), Box<dyn std::error::Error>> {
let client = JcimClient::connect_or_start().await?;
let connection = client
    .open_card_connection(CardConnectionTarget::Reader(ReaderRef::Default))
    .await?;
let session = connection.session_state().await?;
# let _ = session;
connection.close().await?;
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

- APDUs are the message unit of the unified connection API.
- `CardConnection::close()` must be called explicitly if it started and owns a simulation.
- `SimulationInput::Project(...)` is the maintained simulator input.
- Advanced ISO/GP workflows such as channel management, secure messaging, GP auth, install, and admin helpers remain on `JcimClient`.

Examples:

- `cargo run -p jcim-sdk --example satochip_lifecycle`
  - build, start, select, inspect status, and reset the vendored Satochip demo
- `cargo run -p jcim-sdk --example satochip_wallet`
  - build the vendored Satochip project, open a unified `CardConnection`, establish the Satochip
    applet secure channel, create a wallet, derive a BIP32 key, and sign a demo transaction hash
- `cargo run -p jcim-sdk --example satochip_wallet -- --reader "Your Reader Name"`
  - install the built CAP onto a real reader target first, then run the same wallet/signing flow on
    card; if install requires authenticated GP administration, set the matching `JCIM_GP_*`
    environment variables first

These example commands are written to be run from the workspace root.
