# ADR-0109: Perlin and Simplex noise as pure-Handl packages

## Status

proposed

## Context

`perlin_*` and `simplex_*` were previously declared in `rand/rand.hnl` and implemented by the Rust `rand` host module. With `rand` now a pure-Handl contract (`project/adrs/0107-rand-ffi.md`), noise no longer belongs there because:

- Noise is **deterministic**: the same coordinates always produce the same value.
- Noise is **pure**: it requires no `Effect` or `Random` capability.
- Perlin and simplex are distinct algorithms, so they should be separate packages.

`uuid` was already split out as a standalone package. The same contract + implementation pattern should apply to noise, but because the algorithms are entirely pure (no FFI), the `*-c` / `*-stub` split is unnecessary. Each algorithm lives in a single, typed, `export`-based package with its own `.test.hnl` suite.

## Decision

1. Create two self-contained algorithm packages:
   - `handl-packages/perlin/perlin.hnl`
   - `handl-packages/simplex/simplex.hnl`

2. Each package exposes typed, pure arrows with `export`:
   - `perlin_1d: Number -> Number`
   - `perlin_2d: (Number, Number) -> Number`
   - `perlin_3d: (Number, Number, Number) -> Number`
   - `simplex_2d: (Number, Number) -> Number`
   - `simplex_3d: (Number, Number, Number) -> Number`

3. No grades are carried. The arrows use the plain `->` form because they are pure.

4. No `*-c` or `*-stub` backends are created. A general package is enough; any future alternate implementation can be added as `perlin-alt` without touching the contract.

5. The implementation in each package uses top-level `do { ... }` block per exported arrow so that local helpers (`hash`, `smoothstep`, `lerp`, `grad`, etc.) are captured as closures and the package can still use `export` syntax.

6. The deterministic hash is a simple, local 32-bit mix over lattice coordinates, selecting gradients from a small fixed set. This avoids a permutation table and keeps the package small. The outputs are bounded (target `[-1, 1]`) and coordinate-stable.

7. Perlin uses smoothstep `t * t * (3 - 2 * t)` and trilinear interpolation. Simplex uses the classic simplex-grid skew/unskew and kernel summation.

8. Tests are package-local `.test.hnl` files:
   - `handl-packages/perlin/perlin.test.hnl`
   - `handl-packages/simplex/simplex.test.hnl`
   - They import from the local package and use `assert` (as in `dedent/dedent.test.hnl`).
   - Tests cover range, coordinate stability, and boundary behavior.

9. The old `perlin_*` and `simplex_*` declarations in `rand/rand.hnl` are already removed.

## Consequences

- Noise becomes two pure-Handl packages with no Rust code, no `Effect` grade, and no FFI.
- `rand` stays focused on random sampling and distributions.
- The package layout is simpler: one contract/implementation file per algorithm, plus a test file.
- The test surface is `*.test.hnl` only, aligned with the dogfood gate (`hnl test`).
