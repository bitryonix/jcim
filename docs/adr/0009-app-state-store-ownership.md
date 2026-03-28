# ADR 0009: App State Store Ownership

## Status

Accepted

## Decision

JCIM 0.3 keeps the synchronous `AppState` store-helper model in `jcim-app`.

The application continues to use:

- `RwLock` for machine-local config and registry state
- `Mutex` for simulation records, retained build events, and card session state
- the existing `jcimd` `spawn_blocking` bridge for transport-to-application calls

JCIM 0.3 does not actorize `simulations` or `card_sessions`.

The maintained invariants for this model are:

- no lock guard is held across `.await`
- backend handles are cloned out before async work begins
- state commits happen in one bounded synchronous step after async work returns
- failed simulator startup leaves retained failed state and retained events instead of discarding
  the attempt

## Consequences

- The 0.3 transport-neutral app boundary remains synchronous and easier to reason about alongside
  the current daemon bridge.
- Simulation/card-session correctness is protected by explicit store helpers and characterization
  tests rather than by introducing a new runtime model late in the 0.3 cycle.
- Future actorization is deferred beyond 0.3 and would need a fresh decision record plus new
  characterization coverage before changing the ownership model.
