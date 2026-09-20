---
name: workspace-structure
description: Workspace structure rules for the Handl platform. Use when modifying workspace-level configuration, project layout, or Cargo.toml files.
---

# Workspace Structure

Rust workspace + Handl package tree. Root `Cargo.toml` lists members
explicitly — new crates go under `crates/` and are added there.

## Layout

- `crates/` — all Rust code:
  - `plugin-engine-*` — eval/typecheck/lint/test/transform engines (generic,
    data-driven — see the engine-genericity rule)
  - `plugin-ext-*` — `hnl` CLI extensions (fmt, lint, sdkgen, kit, diag)
  - `plugin-lib-*` — native `Module`/`Function` implementations
  - `plugin-sdkgen*` — SDK generation: shared IR (`plugin-sdkgen`),
    backends (`-backend-c`, `-backend-webidl`), emitters (`-emitter-file`)
  - `plugin-ffi` — generic runtime marshalling / dynamic C calls
  - `handl-*` — core language/value/runtime crates
  - `kit-canonical` — assembled canonical kit; `host-cli` → the `hnl` binary
- `handl-packages/` — `.hnl` packages:
  - contract packages (`uuid/index.hnl`, `rand/`, `os/`)
  - backends (`uuid-c`, `uuid-stub`, `os-c`, `rand-c` — `-c` = C FFI,
    `-stub` = deterministic test impl, `-handl` = pure handl)
  - `types/<abi>/` — ABI vocabulary aliases (`types/c/stdint.hnl`, ...)
  - `generated/` — sdkgen output (ports + `<pkg>-c-<target>` adapters)
  - `utils/` — stdlib table sources included via `include_str!` in
    `plugin-lib-stdlib/src/packages.rs`
- `crates/handl-kit-data-canonical/data/` — canonical language **as data**:
  `ast.yaml`, `eval.yaml`, `types.yaml`, `grammar.yaml`, `transforms.yaml`,
  `value_types.yaml`, `representations.yaml`. Language changes are YAML edits,
  never engine edits.
- `project/` — `adrs/`, `notes/`, `tasks/`, `design/`; `docs/` (VitePress);
  `examples/`; `scripts/` (`dogfood-gate.sh`).

## Import resolution

- `from "<spec>" import ...` — file specs resolve relative to the importing
  module's dir, then `module_root`, then CWD; a directory resolves to its
  `index.hnl` (or `<name>.hnl`); bare names fall through to the host module
  chain (`StdlibResolver` in `kit-canonical`, `ModuleSpec` table in
  `plugin-lib-stdlib/src/packages.rs`).

## Dependency direction

- Engines/tooling are generic and data-driven; backend-specific knowledge
  lives in the corresponding `*-backend-*`/`plugin-lib-*` crate — never leak
  it into shared IR or engines.
- `.hnl` packages stay pure handl unless the piece is an irreducible atom
  (`bytes`, `math` bitwise ops); atoms are tiny natives, everything else is
  handl.

## Adding things

- New Rust crate: `crates/<name>` + workspace `members` entry + path deps.
- New stdlib package: `.hnl` source + `ModuleSpec` in `packages.rs` +
  `want("...")` registration in `kit-canonical` (+ `plugin-ext-kit` manifest
  registry if it's kit-visible).
- New language construct: YAML fragment in `data/`, never an engine `match`.
