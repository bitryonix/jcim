# ADR 0008: Managed Paths And Runtime Ownership

## Status

Accepted

## Decision

JCIM separates machine-local concerns into explicit managed roots for:

- config
- durable data and bundled assets
- durable state
- runtime socket and daemon metadata
- logs
- cache

The local daemon owns a versioned runtime metadata file next to the managed socket. Startup and
restart behavior must validate file type and ownership before removing stale runtime artifacts.

Legacy one-root installs are migrated by copying forward durable config and project-registry files
into the split layout while leaving the old root untouched for recovery during the 0.3 cycle.

## Consequences

- Runtime files are treated as ephemeral state, not durable configuration.
- Restart paths fail closed instead of unlinking arbitrary paths.
- Doctor and service-status output must surface the effective managed roots and runtime metadata so
  operators can debug startup and recovery safely.
