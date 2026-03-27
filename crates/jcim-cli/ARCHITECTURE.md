# jcim-cli Architecture

## Intent

`jcim-cli` is the operator-facing shell for the JCIM 0.3 service-first platform.

## Structure

- `main.rs`: thin bootstrap
- `src/cli/args.rs`: Clap command tree and parser-focused tests
- `src/cli/dispatch.rs`: command execution, selector resolution, and typed CLI-only validation
- `src/cli/output/human.rs`: human-readable presentation
- `src/cli/output/json.rs`: JSON envelope/version helpers
- `src/cli/output/mod.rs`: shared output entrypoints that choose human vs machine rendering
- `src/cli/mod.rs`: `run()`, `CliError`, and module wiring

## Dependency direction

- the CLI depends on `jcim-sdk` for local-service discovery, bootstrap, and typed workflows
- business logic stays in `jcim-app` behind the local gRPC contract

## Design notes

- parsing, dispatch, and presentation are separated so automation and human output do not drift
- project/build/run/card/system behavior lives behind the service
- the CLI starts the service when needed, except for `system service status`, which can report a
  stopped service honestly
- `--json` is the stable automation contract for maintained command families and is versioned as
  `jcim-cli.v2`
