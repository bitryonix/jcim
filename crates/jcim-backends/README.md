# jcim-backends

`jcim-backends` adapts the maintained simulator bundle into a single backend command surface.

It owns:

- the backend trait
- the external simulator bundle adapter
- bundle manifest loading
- backend startup handshake and health probing
- the backend actor handle used by the local service and embedded callers

Illustrative low-level use:

```rust
use jcim_backends::backend::BackendHandle;
use jcim_config::config::RuntimeConfig;

let _handle = BackendHandle::from_config(RuntimeConfig::default())?;
# Ok::<(), jcim_core::error::JcimError>(())
```

The maintained backend surface includes:

- `handshake(protocol_version)`
- `backend_health()`
- `get_session_state()`
- typed and raw APDU transmission
- reset, power, logical-channel, secure-messaging, install, delete, list, and snapshot operations
- `shutdown()` for explicit backend teardown

The maintained external bundle contract is newline-delimited JSON, and stateful simulator replies include authoritative ISO session state.
