# Security Policy

JCIM is an internal workbench, but it still handles sensitive surfaces such as machine-local
configuration, helper process invocation, bundled runtimes, and optional GlobalPlatform key
material.

## Report A Concern

Please report suspected security issues privately to the maintainers instead of opening a public
issue with exploit details. Include:

- affected crate or path
- observed behavior
- reproduction steps
- impact assessment
- whether secrets, card state, or bundled assets are involved

## High-Sensitivity Areas

- `jcim-app/src/card.rs` and its future submodules
- GP keyset environment variables: `JCIM_GP_DEFAULT_KEYSET` and `JCIM_GP_<NAME>_{MODE,ENC,MAC,DEK}`
- managed runtime metadata and stale-socket cleanup
- bundled artifacts under `third_party/` and `bundled-backends/`
- simulator/backend process launching and helper command execution

## Handling Secrets

- Keep GP key material in environment variables or an approved secret source only.
- Never log, snapshot, or commit GP key values.
- Error messages may identify missing variable names, but must not echo secret contents.
- Treat helper stderr/stdout as potentially sensitive until reviewed for leakage.

## Scope Of Supported Hosts

The maintained path targets macOS and Linux on both `x86_64` and `aarch64`.
Unsupported-host fallbacks such as `jcim system setup --java-bin /path/to/java` are explicit
operator choices outside that maintained host matrix and must not silently weaken the default
bundled-runtime path on supported hosts.

## Supply-Chain Expectations

- Bundled artifacts must have provenance, checksum, and license metadata in
  `third_party/THIRD_PARTY.toml`.
- Review changes to bundled Java runtimes, helper jars, and simulator bundles as privileged
  changes.
- Keep `cargo audit`, `cargo deny`, and third-party governance tests green in CI.
