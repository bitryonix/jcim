# Daemon Troubleshooting

Use this guide when the local `jcimd` service will not start, reconnect, or clean up correctly.

Maintained contract rules:

- local service package: `jcim.v0_3`
- CLI JSON schema around daemon-facing commands: `jcim-cli.v2`
- managed files: `jcim.toml`, `config.toml`, `projects.toml`, `jcimd.runtime.toml`
- supported maintained hosts: Linux/macOS on `x86_64` and `aarch64`
- unsupported-host Java fallback remains explicit: `jcim system setup --java-bin /path/to/java`
- GP key material stays env-derived and must not appear in daemon diagnostics or JSON error output

## Quick Checks

Inspect the current daemon state without forcing a start:

```sh
cargo run -p jcim-cli -- system service status
```

Inspect the managed paths and Java/runtime selection:

```sh
cargo run -p jcim-cli -- system doctor
```

## Managed Runtime And Socket Paths

Important managed files:

- service socket: under the managed runtime directory (`.../run/jcimd.sock`)
- runtime metadata: `jcimd.runtime.toml` next to the managed socket
- machine-local config: `config.toml`
- machine-local registry: `projects.toml`

`jcim system service status` reports both the socket path and the daemon binary identity captured
at startup. The SDK uses that identity to fail closed when a stale daemon survives across rebuilds.

## Common Failure Modes

### Stale socket after crash

Symptoms:

- service start/connect fails
- socket path exists but nothing is listening

Expected behavior:

- JCIM removes only stale owned sockets
- JCIM refuses to unlink live sockets, regular files, or symlinks at the socket path

Relevant tests:

- `crates/jcimd/tests/runtime_cleanup.rs`
- `crates/jcim-sdk/tests/lifecycle.rs`

### Binary mismatch after rebuild

Symptoms:

- CLI/SDK reports a daemon identity mismatch
- reconnect works only after a restart

Expected behavior:

- the SDK should replace stale runtime metadata and restart against the current `jcimd` binary

### Helper or bundled Java failures

Symptoms:

- builds fail before simulator startup
- card helper commands fail unexpectedly
- doctor output points at an unexpected Java binary

Checks:

- confirm `jcim system doctor` reports the intended runtime
- confirm the bundled runtime exists for the host tuple
- if using an unsupported-host override, verify the explicit `--java-bin` path recorded in
  `config.toml`

Supported maintained hosts:

- Linux `x86_64`
- Linux `aarch64`
- macOS `x86_64`
- macOS `aarch64`

For unsupported hosts, `jcim system setup --java-bin /path/to/java` is the explicit fallback path.
On supported macOS and Linux hosts, JCIM continues to use the bundled runtime and surfaces the
configured fallback path only as a stored override in `config.toml`.

## When To Inspect Files Directly

- inspect `jcimd.runtime.toml` when debugging startup ownership and daemon identity
- inspect `config.toml` when debugging Java/runtime overrides
- inspect `projects.toml` when debugging project discovery or registry drift

These managed files now use atomic writes and should never be hand-edited while the service is
running unless you are debugging a local issue deliberately.
