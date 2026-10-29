---
title: Ports are types; implementations are named for what they do
type: adr
date: 2026-10-29
status: proposed
status_note: 'Proposed — 2026-10-28. Owner: every `lib-*` repo with a Rust shell.'
---

# ADR-0161: Ports are types; implementations are named for what they do

## Context

ADR-0160 fixed the packaging (a `lib-*` repo is a Handl package, not a crate) and
established that a repo whose *implementation* is Rust keeps its crate. Working
through the 18 remaining hybrids showed that "keeps a crate" is not enough: it
does not say **what** the repo is. Today a repo like `lib-base64` mixes three
things — a package source, a metadata table, and (in some cases) real Rust — and
a port like `lib-encoding` mixes an interface with a dispatch helper.

Two rules were missing.

## Decision

**1. A port is types and laws only.** `lib-<port>` declares the shape
(`op`/signature + law today; the categorical form replaces that syntax later) and
carries **no Rust**. A one-line pure-Handl dispatch that belongs to the *shape*
may live there (e.g. `encoding.encode := (v, c) ~> (c.encode)(v)`).

**2. An implementation is a separate repo, named for its language or target.**
`lib-<port>-handl` (pure Handl), `lib-<port>-c` (C FFI), `lib-<port>-native`
(Rust std). Only these may carry a `Cargo.toml`.

**3. Composition is by **imported value**, never by an aggregate.** `encode(v, b64)`
where `b64` comes from `from "base64" import codec`. No `formats = { base64, hex }`
registry: an aggregate would pull every codec into the graph and defeat
tree-shaking.

**4. A shared helper is named for the capability it provides, not for the port it
serves.** If `json-handl` and `toml-handl` both need number/string/list scanning,
the extraction is `lib-literal-handl` — not `lib-parse-handl`, which would imply a
default implementation of the `parse` port that does not exist.

**5. The loader resolves lazily, per module.** Tree-shaking is a loader property:
`module_in_dir(pkg, module)` + `PackageResolver::lazy` read exactly one file, so
importing `base64` never touches `hex`. `pkg.hnl` `deps` therefore describe module
dependencies, not package ones.

## Consequences

- Applied to the current backlog: `lib-parse`, `lib-type`, `lib-encoding` become
  ports; `lib-json-handl`, `lib-base64-handl`, `lib-hex-handl` become impls;
  `lib-base64`, `lib-hex`, `lib-json` stop being repos of their own.
- Class C (the 10 Rust-only backends) resolves the same way: `lib-cmp` keeps the
  axioms, `lib-cmp-handl` or `lib-cmp-native` carries the code.
- Ports never appear in `clone-gate` failures for missing Rust; impls never appear
  in package manifests as crates.
- **Cost:** more repos than "one repo per port" — the price of both tree-shaking
  and a library that can gain a second backend without touching the contract.

## Alternatives rejected

- **One repo per port holding its impl** — the current state; forbids a second
  backend and forces the whole port into every consumer.
- **Aggregate registry (`formats = {…}`)** — simple to write, defeats
  tree-shaking; verified as a requirement by the "import base64, never touch hex"
  test in `handl_plugin_lib_handl::pkg`.
- **Naming impls after the port (`lib-parse-handl`)** — implies a default parser
  that the port's own signature (`parse(input, parser)`) shows does not exist.
