# CLI Reference

From the repository checkout, invoke the CLI as:

```sh
cargo run -p jcim-cli -- <command>...
```

If you have installed the binary separately, replace that prefix with `jcim`.

## Maintained path

The maintained operator path is typed ISO/IEC 7816 and typed GlobalPlatform:

- `jcim sim iso ...`
- `jcim sim gp ...`
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
  - typed GP: `gp auth open`, `gp auth close`, `gp select-isd`, `gp get-status`,
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
- Simulation commands auto-target the current simulation only when exactly one simulation exists.
  Otherwise pass `--simulation <id>` from `jcim sim status`.
- Card commands that touch hardware require a real PC/SC reader, and most of them also require a
  present card. Use `jcim card readers` first, then pass `--reader "Your Reader Name"` when
  needed.
- Hardware install or GP-auth workflows may also require `JCIM_GP_DEFAULT_KEYSET` plus matching
  `JCIM_GP_<NAME>_{MODE,ENC,MAC,DEK}` environment variables.
- `jcim sim start --project <path>` is the maintained simulator entrypoint.
- `jcim card iso ...` and `jcim card gp ...` report observational real-card state; they do not
  pretend to know hidden on-card state JCIM did not directly observe.
