# handl-platform-tests

Integration, end-to-end, and mocked tests for the `handl-platform` crate.

This crate exercises the public platform API without depending on the compiler, VM, or CLI. It uses mock `Plugin` and `Host` implementations to validate the registry, capability discovery, and request dispatch flows in isolation.

## Test files

- `tests/integration.rs` — full plugin registry flow: registration, capability listing, request dispatch.
- `tests/mock.rs` — assertions on the mock `Plugin`/`Host` implementations.
- `tests/e2e.rs` — simulated host loading a `Kit` of two mock plugins and dispatching end-to-end.
