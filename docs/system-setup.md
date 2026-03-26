# System Setup

Persist machine-local JCIM settings with:

```sh
jcim system setup --java-bin java
```

The managed JCIM root is:

- macOS: `~/Library/Application Support/jcim/`
- Linux: `$XDG_DATA_HOME/jcim` or `~/.local/share/jcim/`

Use the doctor command to inspect the effective environment:

```sh
jcim system doctor
```

On macOS, the official simulator path also requires `JCIM_SIMULATOR_CONTAINER_CMD` so JCIM can
launch a Linux-hosted simulator process.
