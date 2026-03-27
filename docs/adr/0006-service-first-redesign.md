# ADR 0006: Service-First Redesign Foundation

## Status

Accepted

## Historical note

This ADR captures the service-first redesign foundation that landed during the 0.2 cycle and still
anchors the current 0.3 baseline. Current contract and runtime details are recorded in ADRs 0007
and 0008.

## Decision

JCIM is redesigned as a local platform centered on one user-local gRPC service, one
transport-neutral application core, and a task-oriented CLI.

## Consequences

- `jcim-app` becomes the application boundary.
- `jcim-api` becomes the public local contract.
- `jcimd` becomes the local gRPC control plane.
- `jcim-cli` becomes a thin client shell.
- Earlier transport shapes, command surfaces, and experimental runtime paths are no longer part of
  the maintained product surface.
- This ADR supersedes the earlier architecture ADRs captured in ADRs 0001 through 0004.
