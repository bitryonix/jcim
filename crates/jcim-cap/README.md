# jcim-cap

`jcim-cap` parses CAP archives and validates imports against export metadata.

It owns:

- in-memory CAP ingestion
- manifest parsing
- package and applet metadata extraction
- import and export validation

Typical use:

```rust
use jcim_cap::export::ExportRegistry;
use jcim_core::model::JavaCardClassicVersion;

let registry = ExportRegistry::new_for_version(JavaCardClassicVersion::V3_0_5);
assert!(!registry.packages().is_empty());
```
