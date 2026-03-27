# ADR 0003: PC/SC Unsafe Boundary

## Status

Historical background for the current safe-by-default workspace

## Historical note

This ADR is restored for traceability. The specific `jcim-pcsc-ifdh` crate named here is no longer
part of the current workspace, but the unsafe-isolation principle still stands.

## Original decision

The IFDH ABI requires raw pointers and C-callable exports, which cannot be implemented without
unsafe Rust.

Keep unsafe code isolated to `jcim-pcsc-ifdh`. The rest of the workspace remains on the safe default
path with workspace linting that forbids unsafe code.

## Consequences

- Unsafe review stays localized.
- The PC/SC bridge can evolve independently from the safe crates.
- FFI correctness still requires ongoing audit and test coverage.
