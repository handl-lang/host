+++
id = "ADR-0128"
title = "ADR-0128: Target Repos — Repo = Target, Module = One Port Implementation"
status = "open"
priority = "medium"
+++

---
title: Target Repos — Repo = Target, Module = One Port Implementation
type: adr
date: 2026-10-15
status: proposed
status_note: Proposed — repo = target/ecosystem, module = one port implementation; extraction landed per the design table.
---

# ADR-0128: Target Repos — Repo = Target, Module = One Port Implementation

## Decision

1. **Repo = target/ecosystem; module = one implementation of one port.** A
   module registers one or more named targets; target name ≠ repo name.
2. **Module directories carry the specific name** (`modules/<port>-<specific>`),
   never the generalized port name: `web/modules/sdkgen-webidl` registers
   `webidl`, and a second sdkgen target lands beside it.
3. **Compiler targets are one repo each** — `dot/`, `term/`, `sexp/`, `handl/`,
   `web/` (JS), `native/` (LLVM, Cranelift) — each holding
   `modules/compile-<target>`.
4. **Platform keeps** ports, core crates, kit machinery, engines, wiring,
   loaders, `host-tooling`, and three machinery `plugin-lib-*` (`handl`, `tag`,
   `scheduler`). Every other `plugin-lib-*` moves to `stdlib/`.
5. **The per-crate extraction map** (26 rows of crate → repo/module) is the
   port plan: see the design doc; each row moves once, with its `.hnl` packages
   pinned to it.
6. **The platform repo is renamed `host/`**; `handl/` means the language target;
   `host-tooling` stays as the shared host contract for `cli/` and `lsp/`.
7. **Hosts sit at the top of the fan**: `host-cli` and `host-lsp` declare
   explicit git-dependency lists, so the registries are load-bearing.
8. **Git deps bypass registries**: consumers set
   `CARGO_NET_GIT_FETCH_WITH_CLI=1`, each repo's CI holds a read credential for
   its siblings, and repos flip public when external contribution matters.
9. **The `lang/` cycle is broken by injection**: consumers take `&Language`/a
   kit handle; the concrete canonical kit is assembled in `kit-canonical/`
   (Phase-0 work, not a late surprise).

## Why

The platform built every target in one graph: `cranelift` and
`clang-sys`/`inkwell` sat in the default build, `plugin-ext-sdkgen` carried 205
foreign Python lines, and nothing had an ownership boundary. Repo-per-target
makes each repo build only its own graph and gives every target an owner.

## Not doing

- **Keep every target in the platform** — the current state: one graph, all
  toolchains, no ownership.
- **One repo per port** — several targets implement one port; they would
  collide.
- **Registry crates instead of git deps** — the targets are private and
  pre-release; git revs are the honest pin.
- **Store the kit in plugin state to break the cycle** — `Kit` is `!Sync`;
  injection is a parameter, not a field.

## Consequences

- Each repo builds only its own graph; `cranelift`/`clang` leave the default
  build and `plugin-ext-sdkgen` loses its foreign lines.
- A port change becomes a multi-PR atomic change plus a git rev bump
  (mitigated by tag pinning, not eliminated).
- `kit-canonical` gains a git dependency on `stdlib/` — a deliberate
  inversion: the platform depends on the stdlib package, not vice versa.
- Every repo needs sibling read credentials; `docs/` stays the public hub.

## Enforcement

- The repos and module paths named in the extraction table (design doc).
- `.cargo/config.toml` — `net.git-fetch-with-cli = true`.
- Each repo's `ci.sh` / bupha config — sibling read credential.

## Design

The extraction table, the `0.8` seam measurements and the cycle analysis:
[`design/0128-target-repos.md`](../design/0128-target-repos.md).
