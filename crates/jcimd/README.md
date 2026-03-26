# jcimd

`jcimd` is the JCIM 0.2 local gRPC control plane.

It hosts one user-local Unix-domain-socket service that manages:

- known projects
- build operations
- simulator operations
- physical-card flows
- machine-local setup and doctor commands

Run it directly with:

```sh
cargo run -p jcimd
```

Most users reach it indirectly through `jcim-cli`, which starts the service on demand.
