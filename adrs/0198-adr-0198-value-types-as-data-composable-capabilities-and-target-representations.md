+++
id = "ADR-0198"
title = "ADR-0198: Value Types as Data — Composable Capabilities and Target Representations"
status = "open"
priority = "med"
+++

---
title: Value Types as Data — Composable Capabilities and Target Representations
type: adr
date: 2026-09-03
status: proposed
status_note: |-
  Proposed. Extends [ADR-0044 — Language as Data and Runtime
  Split](./0044-language-as-data-and-runtime-split.md) from the language
  definition (tokens, grammar, eval schemes) to the **value type system**
  (types, capabilities, target representations). Complements
  [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) and
  [ADR-0197 — Decoupled Trait Layer](./0197-decoupled-trait-layer.md).
  §6 introduces a per-`Language` `typecheck_mode` policy flag
  (`optional` | `gate`) and designates the host language as statically
  gated, building on the typecheck engine from
  [ADR-0054 — Generic Typecheck Engine](./0054-generic-typecheck-engine.md).

  Proposed. Phases 1-2 are the immediate work. Phases 3-4 are the
  near-term goal (closing the perf gap to C/Rust on typed code). Phases
  5-6 are the long-term vision.
---


# ADR-0198: Value Types as Data — Composable Capabilities and Target Representations

> Imported from `kit-data-prelude/project` — originally `ADR-0078` (global registry repo `lang`); renumbered `ADR-0198` in the host merge.

## Decision

1. **Three new data fragment types.** Extend the `Language` data schema (ADR-0044) with three new fragment sets, each loaded from YAML:
2. **Value Types (`value_types.yaml`).** A value type declares:
3. **Capabilities (`capabilities.yaml`).** A capability declares:
4. **Target Representations (`representations.yaml`).** A representation declares how a value type is concretely represented on a specific compilation target:
5. **How the compiler uses this.** The compilation pipeline becomes:
6. **Type inference: gradual + runtime profiling, gated by language policy.** To specialize, the compiler needs to know the concrete type at each operation. The typecheck engine (ADR-0054) is a full dependent typechecker (NBE, unification, bidirectional check/synth) — more powerful than what most "statically typed" languages have. Whether typecheck is a _gate_ (programs that don't typecheck are rejected before eval) or an _optimization_ (best-effort inference, unresolved nodes fall back to dynamic) is **not a property of the engine but a per-`Language` policy**.
7. **Monomorphization.** Yes — the compiler monomorphizes generic operations. When `sum<T: Addable + Zero>` is called with `T = Int`, the compiler:
8. **Relationship to TaggedValue (the `handl-value` crate).** `TaggedValue` is the **dynamic fallback representation** — the equivalent of V8's tagged pointer or CPython's `PyObject*`. It is what the interpreter uses when the compiler has no type information. It is NOT the final representation; it is the baseline that specialization improves upon.
9. **Relationship to ADR-0197 (Decoupled Trait Layer).** ADR-0197 decoupled traits into laws + bundles + handlers. This ADR extends that model:

## Why

ADR-0044 made the _language definition_ pure data: tokens, grammar rules, AST shapes, and eval schemes are YAML files, not Rust trait impls. A `Language` value is serializable, inspectable, authorable as text, and loadable by a runtime with no compile-time knowledge of it. This enabled self-hosting: a `Language` value can describe handl itself.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **User-defined types as first-class citizens.** `Money`, `UUID`,
- **Tiny, generic stdlib.** `sum<T: Addable + Zero>`,
- **No hardcoded knowledge of domain types.** The compiler reads
- **Target-specific representations.** `Int` is `i64` on native,
- **Static specialization.** Where the typechecker proves a type, the

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (mechanism, tables, proofs, trade-offs): [`design/0198-value-types-as-data.md`](../design/0198-value-types-as-data.md).
