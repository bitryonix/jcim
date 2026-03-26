# jcim-cli

`jcim-cli` is the task-oriented shell for the JCIM 0.2 local platform.

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

Further reading:

- Architecture: `ARCHITECTURE.md`
- Workspace docs: `../../README.md`
