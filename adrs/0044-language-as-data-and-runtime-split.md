---
title: Language as Data and Runtime Split
type: adr
date: 2026-09-02
status: accepted
status_note: Accepted — `Language` is serializable data; engines interpret it.
---

# ADR-0044: Language as Data and Runtime Split

## Decision

1. **`Language` is pure serializable data**, built by merging typed fragments
   (`Language::merge`, applied in kit-defined order).
2. **Staged entry points**: each stage consumes only the previous stage's output
   plus the `Language`; `eval` takes an `Ast`, never `Source`.
3. **Crate structure**: each `plugin-engine-*` stage splits into an abstraction
   crate (operation + trait) and an engine crate (interprets data).
4. **Evaluation schemes are data** — the `eval` fragment set maps each `Ast`
   kind to a scheme; engines interpret it. Never closed machine instructions.
5. **Fragment merge distinguishes override from collision** via `id` +
   `replaces: [id]`; no silent last-wins.
6. **Authored data files are canonical.** Rust emitters are a temporary
   scaffold; if they stay, Rust remains the definition and self-hosting dies.
7. **Five review properties are required**: composability (`requires`),
   extensibility, replaceability, performance, safety.

## Why

The language was defined by Rust traits and emitters, so the definition could
not be inspected, serialized or replaced. Self-hosting and multiple engines
require the language to be data; silent last-wins merging made fragment
composition unpredictable.

## Not doing

- **Keep Rust as the definition** — emitters stay scaffold only.
- **Silent last-wins merge** — collisions are errors; overrides are declared.
- **One crate per stage** — abstraction and engine split.
- **Hand-author all ~40 constructs on day one** — emitters prove parity first.

## Consequences
- Data files (`ast/grammar/eval/types/transforms`) become the review surface.
- Emitters are deleted once parity is proven.
- Engines are interchangeable behind the stage trait; kits select them.

## Enforcement
- `kit-prelude-lang/crates/handl-kit-prelude-lang/data/` — the language data.
- `engine-*` — the abstraction/engine split.
- `handl-kit-data-*` — fragment types and merge.

## Design
+Fragment shapes, merge rules, review refinements and the emitter migration:
+[`design/0044-language-as-data-and-runtime-split.md`](../design/0044-language-as-data-and-runtime-split.md).
