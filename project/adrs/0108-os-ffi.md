# ADR-0108: Replace Rust `os` with pure-handl FFI backends

## Status

`proposed`

## Context

The `os` package is currently implemented in two Rust crates:

- `crates/plugin-lib-os` — abstract OS service traits + module adapters + stubs.
- `crates/plugin-lib-os-std` — concrete `std::fs`/`std::process`/`std::net` implementations.

`handl-packages/os/os.hnl` declares the OS interface via `cap`/`op`, and `handl-packages/os_std/os_std.hnl` binds every `op` to a `rust_*` handler. This is the pattern being phased out by ADR-0077.

The surface is large: 29 ops across `fs`, `env`, `path`, `time`, `cmd`, `process`, `tmp`, `socket`, and `sig`. A full single-PR migration is impractical. The work should therefore be split into small, independently testable phases, each producing a pure-handl `os-c` backend over libc/POSIX FFI.

## Decision

1. Keep `os` as the canonical package name and re-structure it as a contract package (`handl-packages/os/index.hnl`) defining an `Os` grade and arrow types for the sub-modules.
2. Create `handl-packages/os-c` as the production backend, implemented over libc/POSIX with the generic `ffi` host capability.
3. Create `handl-packages/os-stub` as the deterministic, no-effect backend for tests and benchmarks.
4. Remove `crates/plugin-lib-os` and `crates/plugin-lib-os-std` once the new `os-c` backend covers the full `os` surface and all kit tests pass.
5. Keep the old `handl-packages/os/os.hnl` and `handl-packages/os_std/os_std.hnl` until the final phase, then delete them and switch package registration.

## Open questions

- `env.keys` and `cmd.run`/`process` require either POSIX `popen`/`fork`/`execve`/`waitpid` FFI or an extension of the generic FFI to read process-environment globals (`environ`) and manage `pid_t` handles. Phase 1 uses the functions already supported by the existing FFI descriptors; later phases may need `handle`-based process/sockets.
- `fs.list` needs `DIR*` and `dirent` reading, which means the FFI `handle` descriptor must be used for `opendir`/`readdir`/`closedir` and `out_struct` for `dirent` data.
- `socket.*` and `sig.*` are POSIX-specific. The contract should document that `os-c` is a POSIX backend; other platforms may need separate backends (`os-win`, `os-posix`, etc.).
- `path.resolve` can either be pure path normalization (no symlink resolution) or call `realpath(3)`. `realpath` allocates and does not fit the current `out_buf` descriptor (it has no `size_t` out-length parameter). A pure normalization implementation will be used first; symlink resolution can be added with a new FFI descriptor or a small `realpath` wrapper later.

## Phases

| Phase | Scope | Files affected |
|---|---|---|
| 1 | `env` (`get`, `has`) and `path` (`join`, `resolve` as normalization, `basename`, `dirname`, `extname`) + `time` (re-export from `time-c`). | `handl-packages/os/index.hnl`, `handl-packages/os-c/env.hnl`, `handl-packages/os-c/path.hnl`, `handl-packages/os-c/time.hnl`, `handl-packages/os-stub/env.hnl`, `handl-packages/os-stub/path.hnl`, `handl-packages/os-stub/time.hnl` |
| 2 | `fs` (`read`, `write`, `exists`, `remove`, `list`) over `open`/`read`/`write`/`unlink`/`opendir`/`readdir`/`closedir`. | `handl-packages/os-c/fs.hnl`, `handl-packages/os-c/errno.hnl`, `handl-packages/os-stub/fs.hnl` |
| 3 | `tmp` (`mkstemp`/`mkdtemp`), `cmd.run`, `process.spawn`/`kill`/`wait`/`status`. | `handl-packages/os-c/tmp.hnl`, `handl-packages/os-c/cmd.hnl`, `handl-packages/os-c/process.hnl`, `handl-packages/os-stub/...` |
| 4 | `socket` and `sig` over `socket`/`connect`/`send`/`recv`/`close`/`kill`. | `handl-packages/os-c/socket.hnl`, `handl-packages/os-c/sig.hnl` |
| 5 | Contract finalization, delete `crates/plugin-lib-os`, `crates/plugin-lib-os-std`, `handl-packages/os/os.hnl`, `handl-packages/os_std/os_std.hnl`, update `crates/plugin-lib-stdlib/src/packages.rs`, `Cargo.toml`, `Cargo.lock`. | `crates/...`, `handl-packages/os/*` cleanup |

## Quality, documentation, and test requirements

1. **Doc comments**: every `export` has a `---` doc comment.
2. **Explicit types**: every `export` has an explicit arrow type; `os/index.hnl` exports named type aliases (`Get`, `Has`, `Join`, `Resolve`, etc.).
3. **Grade safety**:
   - Arrows that perform I/O or read live system state are `~[(Os, Effect)]~>`.
   - `os-stub` arrows are pure.
4. **Test coverage**: one `*.test.hnl` per implementation `.hnl` (e.g. `os-c/env.test.hnl`, `os-c/path.test.hnl`, `os-stub/env.test.hnl`, `os-stub/path.test.hnl`). Tests use the dogfood `assert` form.
5. **Dogfood gate**: all new `.hnl` files must pass `hnl fmt --check`, `hnl lint`, `hnl typecheck`, `hnl test`.
6. **Rust tests**: `crates/kit-canonical/tests/os_c.rs` runs smoke tests against `handl-packages/os-c` loaded as a file package, mirroring `time_c.rs` and `rand_c.rs`.

## Consequences

- `os` becomes a first-class handl package with platform-selectable backends, matching `time` and `rand`.
- No Rust OS logic remains after phase 5.
- The generic `ffi` plugin may need small descriptor extensions for handles, `DIR*`, sockets, and process ids.
- Existing `os.*` consumers continue to work through the old Rust backend until phase 5 completes the switch.

## Related work

- ADR-0077 (stdlib migration to handl)
- ADR-0083 (generic FFI)
- ADR-0107 (rand FFI)
- `feat/ffi-time-c` and `feat/ffi-rand-c`
