+++
id = "ADR-0063"
title = "ADR-0063: Scheme-Core Completeness for Self-Hosting"
status = "open"
priority = "medium"
+++

---
title: Scheme-Core Completeness for Self-Hosting
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed

  Gates [ADR-0044 — Language as Data and Runtime Split](./0044-language-as-data-and-runtime-split.md)
  Phase 8 (self-hosting) and every downstream ADR that assumes handl can describe
  itself as a `Language` value ([0064](./0064-computational-language-authoring.md),
  [0065](./0065-derived-artifact-cache-and-compression.md),
  [0066](./0066-dependency-graph-chunking.md),
  [0067](./0067-chunk-certificates.md)). Refines
  [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) and
  [ADR-0058 — Generic Data-Driven Engines](./0058-generic-data-driven-engines.md).
---


# ADR-0063: Scheme-Core Completeness for Self-Hosting

## Decision


### 1. Self-hosting completeness is a proven property, not an aspiration

Before ADR-0044 Phase 8 is considered started, produce a **completeness proof**:
a `handl-defining-handl` data file (or equivalent test artifact) that satisfies
the three conditions above. The proof is a test, not a document —
`self_host.rs` (already specified in ADR-0044 Phase 8 testing) is the artifact.
This ADR adds the **no-meta-`Foreign`** assertion to that test:

```
assert!(capabilities(handl_language)
    .host_fns
    .iter()
    .all(|name| is_genuine_host_effect(name)));
assert!(!capabilities(handl_language).host_fns.contains("eval"));
assert!(!capabilities(handl_language).host_fns.contains("parse"));
assert!(!capabilities(handl_language).host_fns.contains("compile"));
```

`is_genuine_host_effect` is defined as: the `Foreign` name refers to I/O, FFI,
or capability-tracked host operations (ADR-0046), **not** to language
machinery that belongs in the arrow core.

### 2. Audit and eliminate meta-`Foreign` escapes

Inventory every `Foreign` tag in `eval.yaml`. Classify each as:

- **Genuine host effect** (I/O, FFI, capability-tracked) — keep.
- **Meta-operation masquerading as host function** (`eval`, `parse`,
  `compile`, `var`-lookup, `call`-dispatch, key resolution) — eliminate by
  expressing it as arrow combinators or desugar rules.

The elimination path for each meta-operation:

| Meta-operation        | Elimination path                                                                                                                                                                             |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `eval` (recursive)    | `app` (ADR-0052 §2 — dynamic arrow invocation is the calculus operation, not a host call)                                                                                                    |
| `parse`               | Not an eval operation. Parse is a separate stage (ADR-0044 §2); a self-describing language's eval schemes never invoke parse.                                                                |
| `compile`             | `app` + handler interpretation (ADR-0052 §2 — external interpretation is the handler's job). `compile` as a host function is the Futamura specialization, not a language-internal operation. |
| `var` lookup          | Scope chain as arrow state threaded via `first`/`>>>`; environments are values, not host lookups.                                                                                            |
| `call` dispatch       | `app` (the callee is an arrow value; invocation is `app`).                                                                                                                                   |
| Object key resolution | Desugar rule (ADR-0058 already specifies this migration).                                                                                                                                    |

### 3. Desugar coverage is the primary completeness mechanism

Surface constructs (`match`, `handle`, `pipe`, `ternary`, function literals,
...) lower to core combinators via desugar rules (data rewrites, ADR-0044 §4
"Small core + desugar"). The completeness proof requires that **every surface
construct in the canonical kit has a desugar rule to core combinators**, with
no construct falling back to a `Foreign` escape.

If a construct cannot be desugared without `Foreign`, that is evidence the
core is incomplete — either extend the core (a new combinator, versioned per
ADR-0044 §7 "Scheme combinator versioning") or accept that the construct is
not self-describable and document it as a known gap. Silently using `Foreign`
is not acceptable.

### 4. The conformance suite is the regression gate

ADR-0044's cross-engine conformance suite (`(Language, Ast, expected Value)`
triples) is extended with a **self-description corpus**: triples where the
`Language` is `handl-defining-handl` and the `Ast`/`Value` pairs exercise
handl's own constructs. Every eval engine (tree-walk, bytecode, cranelift)
must produce identical results on this corpus. A divergence is a
self-hosting regression.


## Why

ADR-0044 §"Negative consequences" names the single unsolved risk:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Self-hosting becomes a proven property, not a claim.** The test either
- **The boundary between language and host is sharp.** `Foreign` means host
- **Downstream ADRs are unblocked.** Caching, chunking, and certificates
- **`app` is the sole meta-operation.** `eval`/`compile` as host functions
- **The completeness proof may fail, and fixing it may require core

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0063-scheme-core-completeness.md`](../design/0063-scheme-core-completeness.md).
