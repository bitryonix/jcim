# Third-Party Refresh Process

Use this workflow for any change under `third_party/` or `bundled-backends/`.

## Scope

This applies to:

- bundled Java runtimes
- helper jars
- simulator bundles
- vendored SDK/export trees
- any shipped binary or archive committed in governed asset trees

## Required Update Steps

1. Update the artifact under `third_party/` or `bundled-backends/`.
2. Update the corresponding entry in `third_party/THIRD_PARTY.toml`.
3. Refresh checksums and provenance notes.
4. Update any per-artifact README files if the version or host matrix changed.
5. Run governance checks locally.

## Required Fields In `THIRD_PARTY.toml`

Each entry must keep:

- `name`
- `version`
- `artifact`
- `sha256` for file artifacts
- `license`
- `upstream`
- `update_cadence`
- `notes`

## Verification

Run:

```sh
cargo test -p jcim-config --test third_party_governance
```

If the change also affects the managed Java/runtime path or helper execution, also run:

```sh
cargo test --workspace --all-features
```

## Review Expectations

- keep provenance updates in the same PR as the artifact change
- do not leave `.DS_Store` or editor cruft in governed trees
- document any temporary license/advisory exceptions in `deny.toml` with a reason and expiry
