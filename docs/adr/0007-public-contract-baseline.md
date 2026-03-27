# ADR 0007: JCIM 0.3 Public Contract Baseline

## Status

Accepted

## Decision

The maintained JCIM public baseline is:

- local gRPC package `jcim.v0_3`
- CLI JSON schema `jcim-cli.v2`
- project-backed simulator startup
- project-backed simulator summaries without legacy source-kind or engine-mode surface

The repository does not preserve `jcim.v0_2` or `jcim-cli.v1` in parallel.

Simulator GlobalPlatform controls remain callable as an expert surface, but the maintained
automation compatibility promise covers the task-oriented command families used by project, build,
simulator lifecycle, simulator ISO/APDU, card lifecycle/ISO/GP/APDU, and system flows.

## Consequences

- Docs, examples, and tests must describe the 0.3 baseline explicitly.
- Migration notes replace long-lived compatibility shims for the older local-service and JSON
  envelopes.
- New breaking changes must be versioned deliberately instead of being layered silently onto the
  0.3 baseline.
