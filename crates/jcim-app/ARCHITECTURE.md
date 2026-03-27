# jcim-app Architecture

## Intent

`jcim-app` is the product-facing application layer for JCIM 0.3.

## Responsibilities

- project selection and registry persistence
- project creation and cleanup
- build planning and artifact lookup
- simulator lifecycle management
- physical-card operations
- machine-local setup and diagnostics

## Dependency direction

- `jcim-app` depends on lower-level adapters such as:
  - `jcim-config`
  - `jcim-build`
  - `jcim-backends`
- transport shells depend on `jcim-app`
- lower-level crates do not depend on `jcim-app`
