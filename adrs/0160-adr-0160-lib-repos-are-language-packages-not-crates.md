+++
id = "ADR-0160"
title = "ADR-0160: `lib-*` repos are language packages, not crates"
status = "open"
priority = "med"
+++

---
title: '`lib-*` repos are language packages, not crates'
type: adr
date: 2026-10-28
status: accepted
status_note: |-
  Accepted 2026-11-26 — shipped and extended: atom specs unified to
  `module:` + `atoms:` (`atoms-<module>.yaml` siblings), cdylib atom
  drivers (`libhnl_ext_*`), `pkg.hnl` `name:` = repo minus `lib-`,
  CI moved to `workflows/gate.yaml` + `scripts/hnl-gate.sh`.
  Proposed 2026-10-28; owners: every `lib-*` repo, `host` (loader),
  `automation` (gates).
---


# ADR-0160: `lib-*` repos are language packages, not crates

## Decision


**Treat every `lib-*` repo as a Handl package with a fixed shape, and delete the
Rust shells as the loader learns to fetch deps.**

1. **Repo shape is fixed** (all 136):
   `pkg.hnl` · `src/**/*.hnl` · `README.md` · `LICENSE` (MIT) ·
   `scripts/hnl-gate.sh` · `workflows/gate.yaml` · `bupha.toml`, and no
   `.github/`. `pkg.hnl` declares `name:` as the repo name minus the
   `lib-` prefix.

   **The 89 repos whose payload is `.hnl` carry no `Cargo.toml` and no
   `src/lib.rs`.** The 24 whose *implementation* is Rust — the `-native`, `-c`,
   `-js` backends and `lib-lang-*` substrate adapters — keep their crate: for
   them the crate is the port, not transport. The distinction is mechanical:
   a repo with `.hnl` sources is a package; a repo with none is a backend.

   **Backend suffixes name the binding, not the repo's language.** `-native`
   is an *atom provider* (P-013: an `atoms.yaml` spec — `module:` +
   `atoms:` entries, one module per document with `atoms-<module>.yaml`
   siblings — resolved via `KernelAtomProvider`/
   `registry_from_atoms_kernel`, or a `libhnl_ext_*` cdylib driver —
   implementation-agnostic); `-c` is an *FFI
   binding* into an actual C library; `-js` binds a JS engine. A Rust
   implementation behind the atoms mechanism is `-native`, never `-c` —
   `lib-ip-c` was renamed `lib-ip-native` for exactly this reason.
2. **License is MIT only**, one `LICENSE` file. `LICENSE-APACHE`/`LICENSE-MIT`
   pairs fold into it; `license = "MIT OR Apache-2.0"` disappears with the
   crates.
3. **CI is bupha-only**: a committed `bupha.toml` (`runner.kind =
   "script"`, `status.context = "ci/bupha"`) runs the repo's
   `workflows/gate.yaml` campaign — the shared `handl-gates` bundle via
   `include:` — and `scripts/hnl-gate.sh` is the local `hnl
   typecheck`/`lint`/`test` gate (`ci.sh` is retired fleet-wide).
4. **The shell is a bug, not a feature.** It is removed in the same change that
   gives the loader git deps — never repo-by-repo, because a repo whose shell is
   gone while the loader cannot fetch it becomes unbuildable for its consumers.
5. **Consumers move to packages**: `tool-test` (and any other Rust consumer)
   loads the module through the loader instead of `include_str!`.


## Why

The `lib-*` split (ADR-0128) made each stdlib port a repo. The contract is a Handl package: `pkg.hnl` (`{ deps, modules, entry? }`) plus `.hnl` sources. The intent was explicit — "Rust in stdlib is transitional" (stdlib migration plan §1), and the target layout is:

## Not doing

- **Feature-gate the shells** — Cargo resolves and *fetches* optional/dev git
- **Vendor the `.hnl` into each consumer** — duplicates the source and loses the
- **Publish to crates.io** — out of scope by decision; the whole org installs by

## Consequences

- **Good:** a package is `pkg.hnl` + `.hnl`; adding a port needs no Rust; the
- **Cost:** P1 is loader work in `plugin-lib-handl`, and P2/P3 touch consumers.
- **Risk:** a repo with `pkg.hnl` and no consumer contract (no `axiom` file) is

## Enforcement

- `lib-*` — where this is true.
- `lib-lang-*` — where this is true.
- `tool-test` — where this is true.
- `plugin-lib-handl` — where this is true.
- `lib-pred/src/lib.rs` — where this is true.

## Design

The full record (context, tables, phases, trade-offs): [`design/0160-lib-repos-are-language-packages.md`](../design/0160-lib-repos-are-language-packages.md).
