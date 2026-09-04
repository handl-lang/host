# ADR-0083: Flatten `handl-packages/` Layout

## Status

Accepted and implemented. Supersedes the directory structure implied by
[ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md) §Phase 7
and [ADR-0077 — Stdlib Migration to Handl](./0077-stdlib-migration-to-handl.md)
(which referenced the old `kit-stdlib/data/arrows/` path).

## Context

The stdlib packages lived under a deeply nested path:

```
kit-stdlib/data/arrows/{collection,fn,state,...}/
kit-stdlib/data/contracts/
kit-stdlib/data/{archive,base64,crypto,...}/
packages/cli-framework/
```

This structure had three problems:

1. **`kit-stdlib/data/arrows/` was a semantic boundary baked into the
   LSP and refinement tooling.** The `find_arrows_root` function searched
   for `/arrows/` in file paths to locate the root. Every recursive `.hnl`
   walker, contracts-directory resolver, and moniker generator depended on
   this path segment. Adding a new top-level package required placing it
   under `arrows/` even if it wasn't an "arrow" in the refinement sense.

2. **`kit-stdlib/data/` was an unnecessary parent.** The `kit-stdlib/`
   directory contained only `data/`, which in turn contained all package
   directories. Two levels of indirection with no semantic value.

3. **`packages/` was a separate top-level directory** for non-stdlib
   packages (`cli-framework`), creating two package roots where there
   should be one.

## Decision

Flatten the directory structure so all handl packages live directly under
`handl-packages/`:

```
handl-packages/
  collection/sort.hnl
  fn/map.hnl
  state/counter.hnl
  contracts/list.hnl
  cli-framework/parser.hnl
  archive/archive.hnl
  base64/base64.hnl
  crypto/crypto.hnl
  ...
```

`handl-packages/` is the single root for all handl-authored packages —
stdlib and non-stdlib alike. No `kit-stdlib/`, `data/`, or `arrows/`
intermediate directories.

The `find_arrows_root` function (in `refinement.rs` and `arrow_loader.rs`)
now locates the root by searching for `/handl-packages/` in file paths and
returning everything up to and including that segment. The `contracts/`
directory is a direct child of `handl-packages/`, not a sibling of
`arrows/`.

Recursive `.hnl` walkers across the LSP skip `contracts/` to avoid
treating axiom declaration files as arrow-graph files. The moniker
generator checks for `contracts/` before matching the `handl-packages/`
root, since contracts is now a child of the root rather than a sibling.

## Consequences

### Positive

- **Single package root.** All packages — stdlib, third-party, CLI tools —
  live under one directory. No split between `kit-stdlib/data/` and
  `packages/`.
- **Simpler path resolution.** `find_arrows_root` is a single `find` +
  slice — no package-name segment to skip, no parent traversal.
- **No semantic boundary in the filesystem.** The `arrows/` directory no
  longer exists as a special folder. The distinction between "arrow files"
  and "non-arrow files" is made by the loader (which skips `contracts/`),
  not by the directory structure.
- **Flatter paths.** `handl-packages/collection/sort.hnl` instead of
  `kit-stdlib/data/arrows/collection/sort.hnl` — three segments removed.

### Negative

- **`handl-packages/` is now a flat namespace.** All packages are siblings.
  If a future need arises to group packages (e.g. `handl-packages/stdlib/`
  vs `handl-packages/third-party/`), `find_arrows_root` would need to
  reintroduce package-name skipping. This is acceptable — the function is
  small and centralized.
- **`cli-framework/` is scanned by arrow walkers.** Since it lives under
  `handl-packages/`, the recursive `.hnl` walkers will traverse it. This
  is harmless (cli-framework files are valid `.hnl` that don't contain
  arrow-graph syntax), but if it becomes a problem, a skip-list can be
  added.

## Implementation

- Moved `packages/cli-framework/` → `handl-packages/cli-framework/`.
- Moved `kit-stdlib/data/*` → `handl-packages/*` (removing `kit-stdlib/`
  and `data/` segments).
- Flattened `arrows/collection/`, `arrows/fn/`, `arrows/state/` directly
  into `handl-packages/`.
- Moved `arrows/parse/backtrack.hnl` → `handl-packages/parse-backtrack/backtrack.hnl`
  and `arrows/parse/pratt.hnl` → `handl-packages/parse-pratt/pratt.hnl`
  (to avoid collision with the existing `parse/` package).
- Updated `find_arrows_root` in `refinement.rs` and `arrow_loader.rs`.
- Updated `contracts_dir_from_arrows_root` in `cross_file_def.rs`,
  `type_definition.rs`, `type_hierarchy.rs` (contracts is now a child,
  not a sibling).
- Updated `arrows_root_from_contracts_path` in `implementation.rs`.
- Added `contracts/` skip to 8 recursive walkers across 7 host-lsp files.
- Reordered moniker checks (`contracts/` before root match).
- Updated ~70 `plugin-lib-*` crate `include_str!` paths.
- Updated all LSP test temp-dir structures and hardcoded paths.
- Updated ADR-0069, 0073, 0077, 0079 path references.

Build passes. 349 tests pass; 5 pre-existing failures (refinement parser
`match`/`let` handling) unchanged and unrelated.
