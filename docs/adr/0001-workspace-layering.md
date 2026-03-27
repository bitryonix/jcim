# ADR 0001: Workspace Layering

## Status

Superseded by [ADR 0006](0006-service-first-redesign.md)

## Historical note

This ADR is restored for architectural traceability. Parts of the crate list changed during the
service-first redesign that eventually led to the current 0.3 baseline, but the layering intent
remains useful background.

## Original decision

The original workspace allowed multiple concerns to accumulate inside `jcim-core`, including shared
types, CAP parsing, runtime logic, config, and protocol framing.

Split the workspace into layered crates:

- `jcim-core` for shared types and errors
- `jcim-cap` and `jcim-runtime` for inner policy
- `jcim-config`, `jcim-protocol`, and `jcim-backends` for contracts and adapters
- `jcim-client`, `jcimd`, `jcim-cli`, and `jcim-pcsc-ifdh` for entry points

## Consequences

- Dependency direction is clearer and easier to enforce.
- Changes in transport or process launch code do not force churn in the shared model crate.
- More crates exist, so documentation and crate management discipline become more important.
