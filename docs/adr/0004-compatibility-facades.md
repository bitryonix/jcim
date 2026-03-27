# ADR 0004: Compatibility Facades

## Status

Partially superseded by [ADR 0006](0006-service-first-redesign.md)

## Historical note

This ADR is restored for architectural traceability. The compatibility posture remains relevant,
but the maintained public surfaces are now the service-first CLI and canonical Rust SDK.

## Original decision

The workspace split changes internal boundaries, but existing consumers still expect the established
crate names and convenience methods.

Preserve the current user-facing crate names and keep compatibility wrappers where helpful, while
moving the maintained typed API surfaces into the new inner crates.

## Consequences

- Existing callers keep working with smaller migration pressure.
- The repo can evolve internally without immediately breaking downstream code.
- Compatibility wrappers need periodic review so they do not become permanent clutter.
