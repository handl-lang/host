# ADR-0107: Replace Rust `rand` with pure-handl FFI backends

## Status

`proposed`

## Context

The `rand` package is currently implemented in two Rust crates:

- `crates/plugin-lib-rand` — the active `rand` host module.
- `crates/plugin-lib-rand-native` — an effectively duplicated crate that is also wired into the kit (`handl-plugin-lib-rand-native` appears in `crates/kit-canonical/Cargo.toml` and `crates/plugin-ext-kit/Cargo.toml`).

Both load `handl-packages/rand/rand.hnl`, which declares `cap`/`op`/`handl` arrows and binds them to `rust_*` handlers. This is exactly the pattern the project is migrating away from (ADR-0077): the surface should be authored in `.hnl` and the heavy lifting should be either pure handl over generic primitives or a thin FFI.

A previous spike added `handl-packages/rand-c` and `handl-packages/rand-stub` mirroring `time-c`/`time-stub`, using `ffi.call` to `libc.so.6` `rand`. That version is intentionally minimal and not production-ready: C `rand()` is not cryptographically secure, not thread-safe, unseeded, and has poor distribution.

The goal is a feature-complete, production-ready `rand` package implemented as pure handl with FFI calls to a small number of well-known C libraries, and to remove the hardcoded Rust implementation entirely.

## Decision

1. Keep `rand` as the canonical package name. Re-structure it as a contract package (`handl-packages/rand/index.hnl`) defining the `Random` grade and the full arrow surface (`int`, `float`, `bool`, `pick`, `choice`, `sample`, `shuffle`, `weighted_pick`, `string`, `bytes`, `uuid`, and the seeded + distribution arrows).

2. Replace the Rust backend with `handl-packages/rand-c` implemented over:
   - `libsodium` for CSPRNG: `sodium_init`, `randombytes_random`, `randombytes_uniform`, `randombytes_buf`.
   - `libuuid` for version-4 UUID generation.
   - `libgsl` for statistical distributions (`gauss`, `exponential`, `poisson`, `binomial`) — **or** implement those distributions in pure handl over `libsodium` integers if the stateful `gsl_rng*` requirement proves awkward.
   - Pure handl for `pick`, `choice`, `sample`, `shuffle`, `weighted_pick`, `string`, `perlin_*`, and `simplex_*` on top of the CSPRNG primitives.

3. Keep `handl-packages/rand-stub` as the deterministic, no-effect backend for tests and benchmarks.

4. Remove `crates/plugin-lib-rand`, `crates/plugin-lib-rand-native`, and all `rust_*` handlers from `handl-packages/rand/rand.hnl`. Update `crates/kit-canonical/Cargo.toml` and `crates/plugin-ext-kit/Cargo.toml` to drop the native dependencies.

## Open questions

- Should `libgsl` be used for distributions, or should distributions be implemented in pure handl from `libsodium` integers? `libgsl` is more feature-complete but requires managing a `gsl_rng*` handle across FFI calls. Pure handl removes the C dependency but is more verbose and needs careful testing for bias/correctness.
- Which library for noise? No C noise library is currently available on the host (only `liboslnoise`, which is C++ and hard to call from the generic FFI). Options: (a) keep the existing Rust noise implementation temporarily, (b) implement Perlin/simplex in pure handl, or (c) add a tiny C shim.
- `plugin-lib-rand-native` is duplicated code. Should it be removed before the `rand-c` work or as part of the same PR?

## Plan

| Phase | Scope                                                                                                                                               | Files affected                                                                                                                                                                  |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Sodium primitives in `crand` (`init`, `u32`, `bytes`) and `rand-c` core (`int`, `float`, `bool`, `bytes`, `string`) + validation.                   | `handl-packages/rand-c/crand.hnl`, `handl-packages/rand-c/rand-c.hnl`, `handl-packages/rand-stub/rand-stub.hnl`                                                                 |
| 2     | Pure-handl array ops (`pick`, `choice`, `sample`, `shuffle`, `weighted_pick`).                                                                      | `handl-packages/rand-c/rand-c.hnl`                                                                                                                                              |
| 3     | Distributions (`gauss`, `exponential`, `poisson`, `binomial`) and `libuuid` UUID.                                                                   | `handl-packages/rand-c/rand-c.hnl`, `handl-packages/rand-c/uuid.hnl` (if split)                                                                                                 |
| 4     | Seeded variants and noise (`perlin_*`, `simplex_*`).                                                                                                | `handl-packages/rand-c/seeded.hnl`, `handl-packages/rand-c/noise.hnl` (if split)                                                                                                |
| 5     | Contract (`handl-packages/rand/index.hnl`) and removal of `crates/plugin-lib-rand`, `crates/plugin-lib-rand-native`, plus kit `Cargo.toml` cleanup. | `handl-packages/rand/index.hnl`, delete `crates/plugin-lib-rand`, `crates/plugin-lib-rand-native`, update `crates/kit-canonical/Cargo.toml`, `crates/plugin-ext-kit/Cargo.toml` |

## Quality, documentation, and test requirements

1. **Doc comments**: every `export` in `handl-packages/rand/index.hnl` and the backend packages must have a `---` doc comment explaining behavior, parameters, return value, and examples where non-obvious.

2. **Explicit types**: every `export` must have an explicit arrow type. The contract exports named type aliases (`Int`, `Float`, `Bool`, `Pick`, `Uuid`, etc.) and the backends implement against those aliases, not bare inferred shapes.

3. **Grade safety**:
   - `entropy.bytes` and all unseeded `rand-c` arrows are graded `~[(Random, Effect)]~>`.
   - Seeded and noise arrows are graded `~[Random]~>` (deterministic; no runtime entropy).
   - `rand-stub` arrows are pure (no `Effect`, no `Random`).

4. **Test coverage**: one `*.test.hnl` per implementation `.hnl` (e.g. `rand-c/entropy.test.hnl`, `rand-c/rand-c.test.hnl`, `rand-c/seeded.test.hnl`, `rand-c/noise.test.hnl`, `rand-c/uuid.test.hnl`, `rand-stub/rand-stub.test.hnl`). Tests use `handl-packages/test/assert` and cover, as needed:
   - range validity (`int(0, 10)` in `[0, 10]`, `float(0, 1)` in `[0, 1)`)
   - empty and invalid inputs (`pick []` errors, `sample` count > length errors)
   - `rand-stub` determinism
   - seeded reproducibility (same seed + args → same output)
   - `bytes`/`uuid` format, length, and version
   - distribution sanity (non-negativity, expected bounds)
   - shuffle length preservation and permutation
   - grade-safety where the type system enforces it

5. **Dogfood gate**: the new package files must pass `hnl fmt --check`, `hnl lint`, `hnl typecheck`, and `hnl test` in the dogfood gate.

## Consequences

- `rand` becomes a first-class handl package with platform-selected backends, matching `time`.
- No Rust code for randomness, distributions, or sampling remains.
- Runtime dependency on `libsodium` and `libuuid` (both already present on the host). `libgsl` may become a dependency if phase 3 uses it.
- The `rand` host module disappears; `rand-c` becomes the default production backend.
- All `rand.*` tests currently in `crates/kit-canonical/tests/rand_*.rs` need to be updated or moved to `handl-packages/test` once the implementation is pure handl.

## Related work

- ADR-0077 (stdlib migration to handl)
- `feat/ffi-time-c` (the time-c FFI backend pattern)
