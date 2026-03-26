# jcim-cli Architecture

## Intent

`jcim-cli` is the operator-facing shell for the JCIM 0.2 service-first platform.

## Structure

- `main.rs`: thin bootstrap
- `cli.rs`: Clap command tree and task handlers
- `client.rs`: local service discovery, bootstrap, and Unix-domain-socket gRPC channel setup

## Dependency direction

- the CLI depends on `jcim-api` and a few discovery helpers
- business logic stays in `jcim-app` behind the local gRPC contract

## Design notes

- parsing and presentation live here
- project/build/run/card/system behavior lives behind the service
- the CLI starts the service when needed, except for `system service status`, which can report a
  stopped service honestly
