# ADR-0110: Handl SDK Generator — Windows and macOS C Backend Support

## Status

Proposed.

## Context

[ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md) introduced the
`plugin-sdkgen-backend-c` C / libclang backend and deliberately scoped the first
concrete implementation to Linux x86_64. Windows and macOS support were left for
a later phase because each target requires distinct system-include paths,
shared-library naming, and ABI conventions.

## Decision

Windows and macOS support will be implemented in the existing
`plugin-sdkgen-backend-c` crate by making target-specific behavior data-driven,
not by adding new backends.

- Add target-aware include / sysroot lookup:
  - Linux: `-I/usr/include` and the libclang resource directory.
  - macOS: the macOS SDK path (`xcrun --show-sdk-path` or `SDKROOT`) and
    `-isysroot`.
  - Windows: the MSVC / Clang system include set and Windows-style paths.
- Map the target triple to the correct shared-object name at emitter time:
  - `libc.so.6` for Linux glibc.
  - `libSystem.dylib` for macOS.
  - `*.dll` for Windows.
- Allow the user to pass an explicit target triple
  (`--target x86_64-pc-windows-msvc`, `--target aarch64-apple-darwin`, etc.)
  and validate it during backend construction.
- Preserve the existing port / adapter split and the `SdkgenModule` IR; only
  the parser options and the shared-library name in the adapter change per
  target.

## Consequences

- The C backend becomes genuinely multi-platform without a new crate per OS.
- `.sdkgen.yaml` gains a `target` / `library` section so the shared-object
  mapping is user-overridable.
- Tests must run on CI for each supported target, or via golden snapshots per
  target.
- Linux remains the default and best-supported target.

## References

- [ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md)
- `crates/plugin-sdkgen-backend-c`
