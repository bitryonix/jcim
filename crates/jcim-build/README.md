# jcim-build

`jcim-build` owns JCIM's Java Card source build orchestration:

- source discovery
- typed build requests
- CAP generation
- artifact metadata
- stale-source detection

It is intentionally project-manifest-driven and keeps the CLI thin.
The maintained surface is split so callers can rely on one crate for build planning and execution
without reconstructing fingerprinting or metadata decisions themselves.
