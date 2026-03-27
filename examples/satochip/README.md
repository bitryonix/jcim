# Satochip Example

This example is a source-backed JCIM 0.3 simulator demo built from vendored Satochip Java sources.

## What it demonstrates

- native Java Card source build to CAP
- zero-setup simulator startup from the project manifest on macOS and Linux
- raw APDU exchange against the running simulation
- a Rust wallet flow that selects Satochip, opens the applet secure channel, creates a wallet,
  derives a BIP32 key, and signs a demo transaction hash
- optional CAP install onto a physical card through `jcim card install`
- the same lifecycle through the Rust SDK example

The vendored sources are pinned to upstream commit `8cbaa1d6531df7e20c7a3d47d95766db51d9a136`.

These commands are written to be run from the workspace root.

## Suggested flow

```sh
cargo run -p jcim-cli -- build --project examples/satochip/workdir
cargo run -p jcim-cli -- sim start --project examples/satochip/workdir
cargo run -p jcim-cli -- sim status
```

Select the Satochip applet through the maintained typed ISO path:

```sh
cargo run -p jcim-cli -- sim iso select --aid 5361746F4368697000
```

Use the raw APDU escape hatch only when you want a direct status exchange:

```sh
cargo run -p jcim-cli -- sim apdu B03C000000
```

Reset or stop the simulation:

```sh
cargo run -p jcim-cli -- sim reset
cargo run -p jcim-cli -- sim stop
```

Install the same built CAP onto a physical card:

```sh
cargo run -p jcim-cli -- card install --project examples/satochip/workdir --reader "Your Reader Name"
```

Run the Rust lifecycle demo:

```sh
cargo run -p jcim-sdk --example satochip_lifecycle
```

Run the Rust wallet/bootstrap/signing demo against a fresh virtual Satochip:

```sh
cargo run -p jcim-sdk --example satochip_wallet
```

Run the same wallet/bootstrap/signing demo against a physical reader after installing the built CAP:

```sh
cargo run -p jcim-sdk --example satochip_wallet -- --reader "Your Reader Name"
```

Notes for the physical-card path:

- Use a fresh or disposable card state; the wallet demo performs one-time Satochip setup and seed
  import.
- If your card requires authenticated GP administration for install, configure
  `JCIM_GP_DEFAULT_KEYSET` plus the matching `JCIM_GP_<NAME>_{MODE,ENC,MAC,DEK}` env vars before
  running the example.

Note for managed simulator users:

- No extra simulator setup is required on maintained macOS and Linux hosts. JCIM uses the bundled
  managed-Java simulator path and the vendored Temurin runtime automatically.
- The documented flow uses the maintained simulator lifecycle plus typed ISO/APDU commands. Expert
  simulator GP commands are intentionally omitted from this example path.
- The commands above assume exactly one running simulation. If you have more than one, rerun them
  with `--simulation` and the id from `cargo run -p jcim-cli -- sim status`.
