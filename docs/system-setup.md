# System Setup

From the repository checkout, persist machine-local JCIM settings with:

```sh
cargo run -p jcim-cli -- system setup
```

The managed JCIM root is:

- macOS: `~/Library/Application Support/jcim/`
- Linux: `$XDG_DATA_HOME/jcim` or `~/.local/share/jcim/`

Use the doctor command to inspect the effective environment:

```sh
cargo run -p jcim-cli -- system doctor
```

If you have installed the CLI binary separately, replace the `cargo run -p jcim-cli --` prefix
with `jcim`.

On macOS and Linux, JCIM prefers the repository-bundled Temurin 11 runtime for:

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

`jcim system setup --java-bin /path/to/java` is now an override for unsupported hosts, local
policy, or debugging against a different Java runtime.

The maintained simulator path is project-backed:

```sh
cargo run -p jcim-cli -- build --project examples/satochip/workdir
cargo run -p jcim-cli -- sim start --project examples/satochip/workdir
```
