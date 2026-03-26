# Satochip Example

This example is a source-backed JCIM 0.2 simulator demo built from vendored Satochip Java sources.

## What it demonstrates

- native Java Card source build to CAP
- simulator startup from the project manifest
- raw APDU exchange against the running simulation
- optional CAP install onto a physical card through `jcim card install`
- the same lifecycle through the Rust SDK example

The vendored sources are pinned to upstream commit `8cbaa1d6531df7e20c7a3d47d95766db51d9a136`.

## Suggested flow

```sh
cd examples/satochip/workdir
cargo run -p jcim-cli -- build --project .
cargo run -p jcim-cli -- sim start --project .
cargo run -p jcim-cli -- sim status
```

Select the Satochip applet:

```sh
cargo run -p jcim-cli -- sim apdu 00A40400095361746F4368697000 --simulation sim-...
```

Ask the applet for status:

```sh
cargo run -p jcim-cli -- sim apdu B03C000000 --simulation sim-...
```

Reset or stop the simulation:

```sh
cargo run -p jcim-cli -- sim reset --simulation sim-...
cargo run -p jcim-cli -- sim stop --simulation sim-...
```

Install the same built CAP onto a physical card:

```sh
cargo run -p jcim-cli -- card install --project .
```

Run the Rust lifecycle demo:

```sh
cargo run -p jcim-sdk --example satochip_lifecycle
```

If you want the example to perform the physical-card leg as well, set:

```sh
export JCIM_EXAMPLE_CARD_READER="Your Reader Name"
```
