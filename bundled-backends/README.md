## Bundled Backends

This directory is the packaging root for external JCIM simulator bundles.

Subdirectories:

- `simulator/`: maintained managed-Java simulator bundle slot shipped in this repository

How it is used:

1. `jcim-config::config::RuntimeConfig::backend_bundle_dir()` resolves the selected bundle directory.
2. `jcim-backends::backend::ExternalBackend::spawn()` reads `manifest.toml` from that directory.
3. The manifest supplies the protocol version, JVM main class, classpath, and startup metadata used to launch the simulator helper process.
4. The local service talks to the simulator process over stdin/stdout using the maintained JSON-line backend protocol implemented through `jcim-backends`.
5. Each backend reply carries typed capability data and backend-owned ISO session state so simulator state is reported authoritatively instead of being reconstructed upstream.

What belongs in each bundle directory:

- `manifest.toml`: launcher contract used by `jcimd`
- `classes/` and/or `libs/`: Java classes and jars referenced by the JVM launcher manifest
- any runtime-specific supporting files needed by the simulator launcher

Governance:

- Every shipped binary or jar referenced from this tree must have provenance, license, checksum,
  and update-cadence metadata in `third_party/THIRD_PARTY.toml`.
- Coverage may be exact-file or directory-level when a single manifest entry owns an entire shipped
  subtree such as a vendored runtime or SDK bundle.
- PR CI runs the Rust test suite plus `cargo audit` and `cargo deny check`, and release preflight
  reruns the targeted third-party governance tests directly.
- Tracked OS/editor cruft such as `.DS_Store` is rejected from `third_party/` and
  `bundled-backends/`.
- Contributor updates to bundled jars should land with a manifest update in the same change set.
