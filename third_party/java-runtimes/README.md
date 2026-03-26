# Bundled Java Runtimes

JCIM vendors Temurin JRE archives so the maintained managed-Java path works out of the box on
supported macOS and Linux hosts.

Current bundled version:

- `temurin-11.0.30+7`

Supported host matrix:

- macOS `aarch64`
- macOS `x86_64`
- Linux `x86_64`
- Linux `aarch64`

Vendored archives:

- `temurin-11.0.30+7/OpenJDK11U-jre_aarch64_mac_hotspot_11.0.30_7.tar.gz`
  - SHA-256: `e6bd2ae0053d5768897d2a53e10236bba26bdbce77fab9bf06bfc6a866bf3009`
- `temurin-11.0.30+7/OpenJDK11U-jre_x64_mac_hotspot_11.0.30_7.tar.gz`
  - SHA-256: `fa444f334f2702806370766678c94841a95955f211eed35dec8447e4c33496d1`
- `temurin-11.0.30+7/OpenJDK11U-jre_x64_linux_hotspot_11.0.30_7.tar.gz`
  - SHA-256: `d851e43d81ec6ff7f28efe28c42b4787a045e8f59cdcd6434dece98d8342eb8a`
- `temurin-11.0.30+7/OpenJDK11U-jre_aarch64_linux_hotspot_11.0.30_7.tar.gz`
  - SHA-256: `9d6a8d3a33c308bbc7332e4c2e2f9a94fbbc56417863496061ef6defef9c5391`

Upstream provenance:

- vendor: Eclipse Adoptium / Temurin
- release family: `temurin11-binaries`
- release tag: `jdk-11.0.30+7`

Runtime behavior:

- JCIM verifies the archive checksum before extraction.
- The first managed Java invocation extracts the matching runtime under the managed JCIM root.
- JCIM then reuses that extracted runtime for managed simulator startup, project builds, and
  bundled helper jars on supported macOS and Linux hosts.

This directory contains source artifacts only. Extracted runtime directories are created under the
user's managed JCIM root, not inside `third_party/`.
