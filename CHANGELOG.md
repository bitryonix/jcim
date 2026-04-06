# Changelog

This changelog tracks maintained user-visible and compatibility-relevant changes for JCIM.

## Unreleased

- No unreleased entries yet.

## 0.3.0

- Established the JCIM 0.3 service-first baseline around `jcimd`, `jcim-app`, `jcim-sdk`, and
  `jcim-cli`.
- Fixed the maintained compatibility surfaces for:
  - protobuf package `jcim.v0_3`
  - CLI JSON schema `jcim-cli.v2`
  - managed machine-local files `jcim.toml`, `config.toml`, `projects.toml`,
    and `jcimd.runtime.toml`
- Standardized the project-backed simulator flow, managed Java runtime path, and targeted
  governance tests that protect the maintained contract.
