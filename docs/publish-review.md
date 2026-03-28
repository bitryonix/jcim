# JCIM 0.3 Publish Review

This document records the final documentation and example audit for the JCIM 0.3 publishing pass.
It treats docs, examples, and inline public rustdoc as compatibility surfaces rather than as
informal commentary.

## Source-Of-Truth Matrix

All reviewed docs were checked against these maintained rules:

- protobuf package: `jcim.v0_3`
- canonical protobuf source: `crates/jcim-api/proto/jcim/v0_3/service.proto`
- CLI JSON schema: `jcim-cli.v2`
- managed files: `jcim.toml`, `config.toml`, `projects.toml`, `jcimd.runtime.toml`
- supported maintained hosts: Linux/macOS on `x86_64` and `aarch64`
- unsupported-host Java fallback: `jcim system setup --java-bin /path/to/java`
- GP key material posture: env-derived only, never logged, never shown in JSON errors, never
  documented as CLI flags
- architecture posture: simulator-first, service-first, transport-neutral `jcim-app`, final 0.3
  synchronous `AppState` store-helper model

## Legends

Claim types:

- `C`: contract claim
- `E`: executable command or maintained example command
- `J`: machine-readable example
- `A`: architecture claim
- `O`: operational claim
- `P`: policy/governance claim
- `H`: historical ADR background

Executable snippet classes:

- `deterministic`: expected to run in default local/CI coverage
- `hardware-gated`: requires real reader/card or opt-in hardware env
- `illustrative`: intentionally not presented as a guaranteed maintained workflow
- `none`: no executable snippet

Verification sources:

- `MR`: manual review against code and the source-of-truth matrix
- `DS`: `crates/jcim-cli/tests/docs_smoke.rs`
- `JC`: `crates/jcim-cli/tests/json_contract.rs`
- `DE`: `crates/jcim-sdk/tests/docs_examples.rs`
- `LC`: `crates/jcim-sdk/tests/lifecycle.rs`
- `CH`: `crates/jcim-app/tests/characterization.rs`
- `RC`: `crates/jcimd/tests/runtime_cleanup.rs`
- `BS`: `crates/jcimd/tests/binary_smoke.rs`
- `DC`: `crates/jcim-api/tests/descriptor_contract.rs`
- `TG`: `crates/jcim-config/tests/third_party_governance.rs`
- `RD`: `cargo test --workspace --doc` plus `cargo doc --workspace --no-deps`

## Root Docs

| File | Surface | Claims | Snippets | Verification | Result |
| --- | --- | --- | --- | --- | --- |
| `README.md` | workspace overview, quickstart, compatibility baseline | `C,E,A,O` | deterministic + hardware-gated | `MR,DS,DE,JC,DC` | updated and aligned |
| `CONTRIBUTING.md` | contributor workflow and release-blocking checks | `P,E,C` | deterministic | `MR` | updated and aligned |
| `SECURITY.md` | security posture and secret handling | `P,C,O` | none | `MR` | updated and aligned |
| `LIMITATIONS.md` | current boundaries and future follow-up | `A,O,P` | none | `MR` | updated and aligned |
| `DESIGNDECISIONS.md` | maintained architectural choices | `A,C,P` | none | `MR,DC,CH` | aligned |

## `docs/` Inventory

| File | Surface | Claims | Snippets | Verification | Result |
| --- | --- | --- | --- | --- | --- |
| `docs/api-reference.md` | service contract summary | `C,A,J` | none | `MR,DC,JC` | aligned |
| `docs/architecture-overview.md` | final module/layering map | `A,C` | none | `MR,CH,LC` | aligned |
| `docs/cli-reference.md` | maintained CLI shape and JSON contract | `C,E,J,O` | illustrative + hardware-gated | `MR,DS,JC` | updated and aligned |
| `docs/improvement-roadmap.md` | maintainer-facing completion record | `A,P,O` | none | `MR` | aligned |
| `docs/manifest-reference.md` | manifest keys and example TOML | `C,E` | illustrative | `MR,RD` | aligned |
| `docs/migration-0.3.md` | migration baseline from pre-0.3 surfaces | `C,O,P` | none | `MR,DC,JC` | aligned |
| `docs/release-versioning.md` | release checklist and governance hooks | `P,C,E` | deterministic | `MR` | updated and aligned |
| `docs/system-setup.md` | managed paths, runtime selection, fallback config | `C,E,O` | deterministic | `MR,DS,JC` | updated and aligned |
| `docs/third-party-refresh.md` | third-party/bundled refresh workflow | `P,E,O` | deterministic | `MR,TG` | aligned |
| `docs/troubleshooting-daemon.md` | daemon failure/recovery guide | `O,C,E` | deterministic | `MR,DS,LC,RC` | updated and aligned |
| `docs/publish-review.md` | this audit artifact | `P` | none | `MR` | added in this pass |

## ADR Inventory

Historical ADRs were reviewed for status labeling and factual framing, not as live compatibility
contracts.

| File | Surface | Claims | Snippets | Verification | Result |
| --- | --- | --- | --- | --- | --- |
| `docs/adr/README.md` | ADR index and status map | `A,H,P` | none | `MR` | aligned |
| `docs/adr/0001-workspace-layering.md` | historical layering background | `A,H` | none | `MR` | historical and aligned |
| `docs/adr/0002-backend-actor-model.md` | historical backend-actor decision | `A,H` | none | `MR` | historical and aligned |
| `docs/adr/0003-pcsc-unsafe-boundary.md` | historical unsafe-boundary note | `A,H,P` | none | `MR` | historical and aligned |
| `docs/adr/0004-compatibility-facades.md` | historical compatibility background | `A,H,P` | none | `MR` | historical and aligned |
| `docs/adr/0006-service-first-redesign.md` | foundation ADR for current baseline | `A,H` | none | `MR` | aligned |
| `docs/adr/0007-public-contract-baseline.md` | maintained public baseline | `C,P,A` | none | `MR,DC,JC` | aligned |
| `docs/adr/0008-managed-paths-runtime-ownership.md` | managed-path/runtime ownership policy | `C,O,P` | none | `MR,RC,LC` | aligned |
| `docs/adr/0009-app-state-store-ownership.md` | final 0.3 app-state ownership decision | `A,P,O` | none | `MR,CH,LC` | aligned |

## Crate Docs

Crate README Rust snippets that are not deterministic shell workflows are intentionally labeled as
`illustrative` and were manually checked against the current public API and rustdoc surfaces.

| File | Surface | Claims | Snippets | Verification | Result |
| --- | --- | --- | --- | --- | --- |
| `crates/jcim-api/README.md` | API crate summary | `C,A` | none | `MR,DC` | aligned |
| `crates/jcim-api/ARCHITECTURE.md` | API crate structure | `A,C` | none | `MR,DC` | aligned |
| `crates/jcim-app/README.md` | app boundary summary | `A` | none | `MR,CH` | aligned |
| `crates/jcim-app/ARCHITECTURE.md` | app responsibilities and dependency direction | `A` | none | `MR,CH` | aligned |
| `crates/jcim-backends/README.md` | backend crate summary | `A,E` | illustrative | `MR,RD` | updated and aligned |
| `crates/jcim-backends/ARCHITECTURE.md` | backend actor/bundle structure | `A` | none | `MR` | aligned |
| `crates/jcim-build/README.md` | build crate summary | `A` | none | `MR` | aligned |
| `crates/jcim-build/ARCHITECTURE.md` | build crate structure | `A` | none | `MR` | aligned |
| `crates/jcim-cap/README.md` | CAP parsing summary | `A,E` | illustrative | `MR,RD` | updated and aligned |
| `crates/jcim-cap/ARCHITECTURE.md` | CAP crate structure | `A` | none | `MR` | aligned |
| `crates/jcim-cli/README.md` | CLI crate summary | `A,C` | none | `MR,JC` | aligned |
| `crates/jcim-cli/ARCHITECTURE.md` | CLI module layout and JSON contract note | `A,C` | none | `MR,JC` | aligned |
| `crates/jcim-config/README.md` | config crate summary | `A,E` | illustrative | `MR,RD` | updated and aligned |
| `crates/jcim-config/ARCHITECTURE.md` | config crate structure | `A` | none | `MR` | aligned |
| `crates/jcim-core/README.md` | core crate summary | `A,E` | illustrative | `MR,RD` | updated and aligned |
| `crates/jcim-core/ARCHITECTURE.md` | core crate structure | `A` | none | `MR,RD` | aligned |
| `crates/jcim-sdk/README.md` | SDK overview and example entrypoints | `A,E,O` | illustrative + deterministic + hardware-gated | `MR,DE,LC,RD` | updated and aligned |
| `crates/jcim-sdk/ARCHITECTURE.md` | SDK module layout and bootstrap split | `A` | none | `MR,LC` | aligned |
| `crates/jcimd/README.md` | daemon crate summary and direct launch path | `A,E` | deterministic | `MR,BS` | aligned |
| `crates/jcimd/ARCHITECTURE.md` | daemon transport/runtime ownership | `A,O` | none | `MR,RC` | aligned |

## Examples, Bundled Assets, And Third-Party Docs

| File | Surface | Claims | Snippets | Verification | Result |
| --- | --- | --- | --- | --- | --- |
| `examples/satochip/README.md` | maintained example workflow | `E,O,A` | deterministic + hardware-gated | `MR,DS,DE` | updated and aligned |
| `bundled-backends/README.md` | bundled-backend packaging/governance | `A,P,O` | none | `MR,TG` | aligned |
| `bundled-backends/simulator/README.md` | maintained simulator bundle posture | `A,C,O` | none | `MR,TG` | updated and aligned |
| `third_party/java-runtimes/README.md` | bundled runtime host matrix and provenance | `C,O,P` | none | `MR,TG` | aligned |

## Inline Rustdoc Review

Reviewed public rustdoc surfaces:

- `crates/jcim-api/src/lib.rs`
- `crates/jcim-app/src/lib.rs`
- `crates/jcim-core/src/lib.rs`
- `crates/jcim-core/src/model.rs`
- `crates/jcim-sdk/src/lib.rs`
- `crates/jcim-sdk/src/connection.rs`
- `crates/jcimd/src/lib.rs`

Results:

- public crate/module summaries describe the post-refactor 0.3 structure rather than pre-split
  files
- public verification-sensitive crate docs now point at the integration suites that protect the
  maintained workflow
- internal comments reviewed in touched areas remain aligned with the final invariants:
  no guard across `.await`, clone handles before async work, reserve -> startup -> commit/fail

Rustdoc gate for this pass:

- `cargo test --workspace --doc`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`

## Unresolved Issues

- None after the final publishing gate.

## Final Gate Results

Passed in this publishing pass:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace --doc`
- `cargo test -p jcim-api --test descriptor_contract`
- `cargo test -p jcim-app --test characterization -- --test-threads=1`
- `cargo test -p jcim-cli --test docs_smoke -- --test-threads=1`
- `cargo test -p jcim-cli --test json_contract -- --test-threads=1`
- `cargo test -p jcim-sdk --test docs_examples`
- `cargo test -p jcim-sdk --test lifecycle -- --test-threads=1`
- `cargo test -p jcimd --test runtime_cleanup -- --test-threads=1`
- `cargo test -p jcim-config --test third_party_governance`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`

## Publish Verdict

Ready for JCIM 0.3 publishing from the docs, examples, and rustdoc surface perspective.
