# jcim-cap Architecture

## Intent

`jcim-cap` isolates CAP parsing and export resolution from runtime execution and transport code.

## Main modules

- `cap`: CAP archive parsing and profile-aware validation
- `export`: export registry and import resolution helpers
- `prelude`: convenience re-exports

## Dependency direction

- Depends only on `jcim-core`.
- Does not depend on service code, config loading, or runtime state management.

## Design notes

- Parsing is pure Rust and in memory.
- CAP validation uses selected Java Card profile information from `jcim-core` model types.
