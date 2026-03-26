# CLI Reference

JCIM 0.2 exposes these top-level task groups:

- `jcim project new`
- `jcim project show`
- `jcim project clean`
- `jcim build`
- `jcim build artifacts`
- `jcim sim start`
- `jcim sim stop`
- `jcim sim status`
- `jcim sim logs`
- `jcim sim apdu`
- `jcim sim reset`
- `jcim card readers`
- `jcim card status`
- `jcim card install`
- `jcim card delete`
- `jcim card packages`
- `jcim card applets`
- `jcim card apdu`
- `jcim card reset`
- `jcim system setup`
- `jcim system doctor`
- `jcim system service status`

Use `--json` on any command for structured output.

Notable CLI input shapes:

- `jcim sim start` accepts either `--project` / `--id` or `--cap`
- `jcim card install` accepts either `--project` / `--id` or `--cap`
