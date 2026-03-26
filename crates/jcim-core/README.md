# jcim-core

`jcim-core` is the shared model crate for JCIM.

It owns:

- `Aid`
- APDU value objects
- Java Card profile and hardware model types
- runtime summary and install result types
- shared typed errors
- convenience re-exports through `prelude`

Further reading:

- Architecture: `ARCHITECTURE.md`
- Workspace-level tradeoffs: `../../LIMITATIONS.md` and `../../DESIGNDECISIONS.md`

Typical use:

```rust
use jcim_core::model::{CardProfile, CardProfileId};

let profile = CardProfile::builtin(CardProfileId::Classic305);
assert_eq!(profile.version.display_name(), "3.0.5");
```
