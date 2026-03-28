# Contributing To JCIM

JCIM is maintained as a service-first, simulator-first Java Card workbench. Contributions should
preserve that center of gravity and prefer small, reviewable changes over broad rewrites.

## Ground Rules

- Preserve the maintained baselines: protobuf package `jcim.v0_3`, CLI JSON schema `jcim-cli.v2`,
  project-backed simulator startup, and the managed-Java simulator path.
- Keep `jcim-app` as the application boundary, `jcimd` as the single local control plane, and
  `jcim-cli` as a thin shell.
- Do not weaken the `unsafe` ban, lint posture, or docs expectations.
- Prefer characterization tests before large internal refactors.
- Do not casually change CLI flags, JSON `kind` values, protobuf service names/field numbers, or
  managed file names.

## Local Workflow

Typical setup:

```sh
cargo run -p jcim-cli -- system setup
cargo run -p jcim-cli -- system doctor
```

Recommended verification before opening a PR:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Targeted contract/governance checks that are expected to stay review-blocking:

```sh
cargo test -p jcim-api --test descriptor_contract
cargo test -p jcim-app --test characterization -- --test-threads=1
cargo test -p jcim-cli --test docs_smoke -- --test-threads=1
cargo test -p jcim-cli --test json_contract -- --test-threads=1
cargo test -p jcim-sdk --test docs_examples
cargo test -p jcim-sdk --test lifecycle -- --test-threads=1
cargo test -p jcimd --test runtime_cleanup -- --test-threads=1
cargo test -p jcim-config --test third_party_governance
```

For the final publishing pass, also run:

```sh
cargo test -p jcim-api --test descriptor_contract
cargo test --workspace --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Compatibility Rules

- Treat `crates/jcim-api/proto/jcim/v0_3/service.proto` as governed public surface.
- Treat CLI `--json` output as a stable automation surface for maintained command families.
- Keep `jcim.toml`, `config.toml`, `projects.toml`, and `jcimd.runtime.toml` stable unless the
  change is versioned and documented.
- If a change would alter public behavior, update docs and migration notes in the same change set.

## Third-Party And Bundled Assets

- Updates under `third_party/` or `bundled-backends/` must update
  `third_party/THIRD_PARTY.toml` in the same PR.
- Refresh checksums, provenance notes, and update cadence when changing bundled assets.
- Run `cargo test -p jcim-config --test third_party_governance` after those changes.
- See [`docs/third-party-refresh.md`](docs/third-party-refresh.md) for the exact refresh workflow.

## Secrets And Diagnostics

- Never commit GP key material, test cards, credentials, or production secrets.
- Do not paste `JCIM_GP_*` secret values into docs, snapshots, or failure assertions.
- Diagnostics may mention variable names such as `JCIM_GP_DEFAULT_KEYSET`, but must not contain
  env-derived key values.

## Good Change Shape

- Keep diffs tightly scoped.
- Add or update tests for bug fixes and behavioral refactors.
- Keep docs aligned with implementation and CI.
- Prefer moving code along real responsibility seams, not arbitrary file splitting.
