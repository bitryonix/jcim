# System Setup

From the repository checkout, persist machine-local JCIM settings with:

```sh
cargo run -p jcim-cli -- system setup
```

Maintained contract rules:

- local service package: `jcim.v0_3`
- CLI JSON schema: `jcim-cli.v2`
- managed files: `jcim.toml`, `config.toml`, `projects.toml`, `jcimd.runtime.toml`
- supported maintained hosts: Linux/macOS on `x86_64` and `aarch64`
- unsupported-host Java fallback remains explicit: `jcim system setup --java-bin /path/to/java`
- GP key material stays env-derived and must not be written into managed files or logs

The managed JCIM layout is:

- macOS:
  - config: `~/Library/Application Support/jcim/config/`
  - durable state: `~/Library/Application Support/jcim/state/`
  - runtime socket/state: `~/Library/Application Support/jcim/run/`
  - logs: `~/Library/Logs/jcim/`
  - cache: `~/Library/Caches/jcim/`
  - extracted runtime assets: `~/Library/Application Support/jcim/data/bundled/`
- Linux:
  - config: `$XDG_CONFIG_HOME/jcim` or `~/.config/jcim/`
  - durable data/assets: `$XDG_DATA_HOME/jcim` or `~/.local/share/jcim/`
  - durable state/logs: `$XDG_STATE_HOME/jcim` or `~/.local/state/jcim/`
  - runtime socket/state: `$XDG_RUNTIME_DIR/jcim` or `~/.local/state/jcim/run/`
  - cache: `$XDG_CACHE_HOME/jcim` or `~/.cache/jcim/`

Use the doctor command to inspect the effective environment:

```sh
cargo run -p jcim-cli -- system doctor
```

Use service status to inspect the current socket path and daemon identity without starting the
service:

```sh
cargo run -p jcim-cli -- system service status
```

If you have installed the CLI binary separately, replace the `cargo run -p jcim-cli --` prefix
with `jcim`.

On supported macOS and Linux hosts, JCIM uses the repository-bundled Temurin 11 runtime for:

- Java Card builds
- managed simulator startup
- bundled helper jars used by physical-card workflows

That means the maintained simulator path requires:

- no Docker
- no `JCIM_SIMULATOR_CONTAINER_CMD`
- no host `java` install
- no `JAVA_HOME`

The vendored runtime archives live under:

- `third_party/java-runtimes/temurin-11.0.30+7`

The first managed Java invocation extracts the matching runtime under the managed JCIM root and
reuses it afterward. `jcim system doctor` reports both the configured Java path and the effective
runtime that JCIM will actually use.

If an older JCIM checkout left `config.toml` or `projects.toml` under the legacy one-root layout,
the current app copies those files forward into the split layout on first boot and leaves the old
files untouched for recovery.

Migration details for the 0.3 baseline live in [`migration-0.3.md`](migration-0.3.md).

`jcim system setup --java-bin /path/to/java` persists the unsupported-host fallback Java path in
`config.toml`. On supported macOS and Linux hosts, `jcim system doctor` still reports the bundled
Temurin runtime as the effective Java runtime while showing the configured fallback path
separately.

The maintained simulator path is project-backed:

```sh
cargo run -p jcim-cli -- build --project examples/satochip/workdir
cargo run -p jcim-cli -- sim start --project examples/satochip/workdir
```
