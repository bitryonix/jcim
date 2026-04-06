# Release And Versioning Notes

JCIM 0.3 currently treats these as the maintained public baselines:

- protobuf package `jcim.v0_3`
- CLI JSON schema `jcim-cli.v2`
- project-backed simulator startup
- managed machine-local files: `jcim.toml`, `config.toml`, `projects.toml`, `jcimd.runtime.toml`

## When A Version Bump Is Required

Plan a deliberate version bump when changing any of the following in a breaking way:

- protobuf package name, service names, or field numbers
- CLI JSON `schema_version`, `kind`, or stable payload keys
- manifest keys or managed machine-local file names/formats
- public `jcim-sdk` types or method signatures used by maintained consumers

Internal refactors that preserve the existing surface do not require a public version bump.

## Release Readiness Checklist

- run fmt, clippy, workspace tests, doctests, and rustdoc checks
- run JSON contract, docs smoke, SDK docs examples, descriptor compatibility, app
  characterization, SDK lifecycle, daemon runtime-cleanup, and third-party governance tests
- keep `cargo audit` and `cargo deny check` green
- update `CHANGELOG.md` for maintained user-visible or compatibility-relevant changes
- update docs and migration notes for any user-visible behavior change
- verify `third_party/THIRD_PARTY.toml` matches committed bundled assets

## Current Governance Hooks

- `crates/jcim-api/tests/descriptor_contract.rs`
- `crates/jcim-cli/tests/docs_smoke.rs`
- `crates/jcim-cli/tests/json_contract.rs`
- `crates/jcim-app/tests/characterization.rs`
- `crates/jcim-sdk/tests/docs_examples.rs`
- `crates/jcim-sdk/tests/lifecycle.rs`
- `crates/jcim-config/tests/third_party_governance.rs`
- `crates/jcimd/tests/runtime_cleanup.rs`

These tests are intended to block accidental drift in maintained compatibility surfaces.
