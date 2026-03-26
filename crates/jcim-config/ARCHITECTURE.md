# jcim-config Architecture

## Intent

`jcim-config` translates operator-facing configuration into typed runtime inputs.

## Main modules

- `config`: runtime config, backend config, GP key config, TOML parsing, and profile resolution
- `prelude`: convenience re-exports

## Dependency direction

- Depends on `jcim-core`.
- Does not depend on CAP parsing, runtime execution, wire framing, or CLI parsing.

## Design notes

- CLI parsing stays outside this crate.
- Config loading is kept explicit so binaries can fail fast on invalid configuration.
