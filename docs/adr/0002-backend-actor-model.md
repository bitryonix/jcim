# ADR 0002: Backend Actor Model

## Status

Superseded by [ADR 0006](0006-service-first-redesign.md)

## Historical note

This ADR is restored for architectural traceability. The current simulator backend still benefits
from the same message-boundary thinking even though the concrete implementation evolved.

## Original decision

The daemon previously relied on shared mutable backend state patterns that risked lock-across-`await`
behavior and mixed backend execution with transport concerns.

Run backend operations behind a bounded actor channel owned by `jcim-backends`. The daemon and the
embedded client send commands to that actor instead of touching backend state directly.

## Consequences

- Async callers avoid holding backend locks across `.await`.
- The builtin runtime remains single-threaded.
- External process integration and builtin runtime execution share one command boundary.
