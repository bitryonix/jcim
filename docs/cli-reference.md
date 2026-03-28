# CLI Reference

From the repository checkout, invoke the CLI as:

```sh
cargo run -p jcim-cli -- <command>...
```

If you have installed the binary separately, replace that prefix with `jcim`.

Maintained contract rules:

- CLI JSON schema: `jcim-cli.v2`
- service package beneath the CLI: `jcim.v0_3`
- managed files: `jcim.toml`, `config.toml`, `projects.toml`, `jcimd.runtime.toml`
- supported maintained hosts: Linux/macOS on `x86_64` and `aarch64`
- unsupported-host Java fallback remains explicit: `jcim system setup --java-bin /path/to/java`
- GP key material is env-derived and must not appear in human-readable logs or JSON error output

## Maintained path

The maintained simulator path is:

- `jcim sim start|status|logs|reset|stop`
- `jcim sim iso ...`
- `jcim sim apdu`

The maintained machine-readable automation contract currently covers:

- `jcim project ...`
- `jcim build ...`
- `jcim sim start|status|logs|reset|stop`
- `jcim sim iso ...`
- `jcim sim apdu`
- `jcim card readers|status|install|delete|packages|applets|reset`
- `jcim card iso ...`
- `jcim card gp ...`
- `jcim card apdu`
- `jcim system setup|doctor|service status`

The maintained physical-card admin path is:

- `jcim card iso ...`
- `jcim card gp ...`

Raw APDU passthrough remains available as the expert escape hatch:

- `jcim sim apdu`
- `jcim card apdu`

## Top-level groups

- `jcim project ...`
  - `new`
  - `show`
  - `clean`
- `jcim build ...`
  - `build`
  - `build artifacts`
- `jcim sim ...`
  - lifecycle: `start`, `status`, `logs`, `reset`, `stop`
  - typed ISO: `iso status`, `iso select`, `iso channel-open`, `iso channel-close`,
    `iso secure-open`, `iso secure-advance`, `iso secure-close`
  - expert GP surface: `gp auth open`, `gp auth close`, `gp select-isd`, `gp get-status`,
    `gp set-card-status`, `gp set-application-status`, `gp set-security-domain-status`
  - raw escape hatch: `apdu`
- `jcim card ...`
  - reader and inventory: `readers`, `status`, `packages`, `applets`
  - install and delete: `install`, `delete`
  - lifecycle: `reset`
  - typed ISO: `iso status`, `iso select`, `iso channel-open`, `iso channel-close`,
    `iso secure-open`, `iso secure-advance`, `iso secure-close`
  - typed GP: `gp auth open`, `gp auth close`, `gp select-isd`, `gp get-status`,
    `gp set-card-status`, `gp set-application-status`, `gp set-security-domain-status`
  - raw escape hatch: `apdu`
- `jcim system ...`
  - `setup`
  - `doctor`
  - `service status`

## Important behavior

- Use `--json` on any command for structured output.
- JSON output is the stable automation surface. Success payloads include
  `schema_version = "jcim-cli.v2"` plus a stable `kind` marker while keeping the existing payload
  keys at the top level. JSON-mode failures go to `stderr` with the same version marker.
- The stability guarantee applies to the maintained command families listed above, not to every
  experimental or expert-only path.
- Human-readable output remains operator-facing and is not the automation contract.
- Simulation commands auto-target the current simulation only when exactly one simulation exists.
  Otherwise pass `--simulation <id>` from `jcim sim status`.
- Card commands that touch hardware require a real PC/SC reader, and most of them also require a
  present card. Use `jcim card readers` first, then pass `--reader "Your Reader Name"` when
  needed.
- Hardware install or GP-auth workflows may also require `JCIM_GP_DEFAULT_KEYSET` plus matching
  `JCIM_GP_<NAME>_{MODE,ENC,MAC,DEK}` environment variables.
- Published reader-backed examples are hardware-gated. The default deterministic smoke suite keeps
  them opt-in and only exercises them when `JCIM_HARDWARE_TESTS=1`.
- `jcim sim start --project <path>` is the maintained simulator entrypoint.
- `jcim sim gp ...` remains available as an expert surface for simulator backends that expose a
  compatible security domain, but it is not the default simulator lifecycle path and is not part
  of the current stable JSON compatibility promise.
- `jcim card iso ...` and `jcim card gp ...` report observational real-card state; they do not
  pretend to know hidden on-card state JCIM did not directly observe.

Example success envelope:

```json
{
  "schema_version": "jcim-cli.v2",
  "kind": "system.service_status",
  "running": false,
  "socket_path": "/path/to/jcimd.sock"
}
```

Example error envelope:

```json
{
  "schema_version": "jcim-cli.v2",
  "kind": "error",
  "message": "no artifact metadata found for this project; run `jcim build` first"
}
```
