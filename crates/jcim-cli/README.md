# jcim-cli

`jcim-cli` is the task-oriented shell for the JCIM 0.3 local platform.

It is intentionally thin:

- it discovers the managed local service
- it starts that service when needed
- it calls `jcim-sdk`, which speaks the local gRPC contract
- it renders task-oriented output for operators and `--json` output for automation

Primary commands:

- `jcim project ...`
- `jcim build ...`
- `jcim sim ...`
- `jcim card ...`
- `jcim system ...`

Maintained task paths:

- typed simulator workflows: `jcim sim iso ...`
- typed physical-card workflows: `jcim card iso ...` and `jcim card gp ...`
- raw APDU passthrough: `jcim sim apdu` and `jcim card apdu` as the expert escape hatch
- simulator GP workflows remain available as an expert surface, but they are not part of the
  stable automation guarantee yet

Further reading:

- Architecture: `ARCHITECTURE.md`
- Workspace docs: `../../README.md`
