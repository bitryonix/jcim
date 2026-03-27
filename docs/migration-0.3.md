# JCIM 0.3 Migration Notes

JCIM 0.3 is the maintained baseline for the current workspace.

## Public contract changes

- The local gRPC contract is `jcim.v0_3`.
- Simulator startup is project-backed only.
- Maintained simulator summaries no longer expose the older simulator source-kind and engine-mode
  fields from the pre-0.3 surface.
- CLI JSON output is versioned as `jcim-cli.v2`.

## CLI automation changes

- JSON success and error envelopes include `schema_version = "jcim-cli.v2"` and a stable `kind`
  marker.
- The compatibility promise currently applies to the maintained task-oriented command families:
  `project`, `build`, simulator lifecycle, simulator ISO/APDU, card lifecycle/ISO/GP/APDU, and
  `system`.
- Expert simulator GP commands remain available, but they are not part of the stable automation
  guarantee yet.

## Managed path changes

- Machine-local config, durable data/state, runtime socket/metadata, logs, and cache now live in
  separate managed roots.
- If an older checkout left `config.toml` or `projects.toml` in the legacy one-root layout, JCIM
  copies those files forward on first boot and leaves the old files untouched for recovery.

## Operator notes

- On supported macOS and Linux hosts, the maintained path uses the bundled Temurin runtime and
  managed simulator bundle. No Docker, `JCIM_SIMULATOR_CONTAINER_CMD`, or host `JAVA_HOME` is
  required.
- `jcim system doctor` reports the effective managed roots and runtime selection.
- `jcim system service status` reports the current socket path and daemon binary identity without
  starting the service.
