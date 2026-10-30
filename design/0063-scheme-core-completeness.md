---
title: Scheme-Core Completeness for Self-Hosting — design detail
type: design
date: 2026-09-02
updated: 2026-10-29
refs: [ADR-0063]
---

# Scheme-Core Completeness for Self-Hosting — design detail

Moved verbatim from ADR-0063 (ADR-0161 size budget). The ADR carries the rules;
this page carries the context, tables, phases and trade-offs.

## Context

ADR-0044 §"Negative consequences" names the single unsolved risk:

> The scheme combinator set + desugar rules are the crux and the risk.
> "Semantics as data" for a full language (call, match, handle, pipes, ternary,
> function literals, ...) is genuinely hard. The core combinator set must be
> small enough to interpret/compile efficiently yet expressive enough that
> surface constructs desugar to it without escaping to Rust. An incomplete
> combinator set or desugar layer forces `Foreign` escapes, which undermine the
> pure-data model.

ADR-0052 replaced the lambda-flavored `Scheme` with the arrow-calculus kernel
(`arr`/`>>>`/`first`/`choice`/`loop`/`app`) and removed `vau`. ADR-0058
established that engines are fixed interpreters of data and that
language-specific logic must migrate to desugar rules or scheme descriptions,
not engine code.

Neither ADR proves the property that matters: **the arrow-calculus core +
desugar rules can describe handl's own semantics without `Foreign` escapes.**
Until that property holds, self-hosting (ADR-0044 Phase 8) is not real — a
`handl-defining-handl` data file that needs `Foreign("parse")` or
`Foreign("eval")` to describe its own evaluation is not self-hosting; it is
delegation to the host under a different name.

This is the gate. Caching, chunking, compression, certificates
([0065](./0065-derived-artifact-cache-and-compression.md)–[0067](./0067-chunk-certificates.md))
are packaging and trust optimizations on a `Language` value. They are
meaningless if the `Language` value cannot yet describe handl itself.

### What "completeness" means precisely

A `Language` value `L` is **self-describing** if:

1. `eval(L, ast_of_L)` produces `L` (or an equivalent `Language` value) — the
   language can evaluate the data that defines it.
2. `capabilities(L)` contains no `Foreign` names that refer to host-internal
   parsing, evaluation, or compilation machinery. `Foreign` is reserved for
   genuine host effects (I/O, FFI), per ADR-0044 §4 "Foreign as data, not Rust
   escape" and ADR-0046's capability model.
3. Every surface construct in the canonical kit desugars to core arrow
   combinators (`arr`/`>>>`/`first`/`choice`/`loop`/`app`) plus `Prim` ops
   whose implementations are themselves expressible as arrow combinators (or
   are genuine host effects, not language semantics).

Condition 2 is the sharp edge. `Foreign("eval")` in a self-describing language
is a category error: `eval` is `app` (dynamic arrow invocation, ADR-0052 §2),
not a host function. If the self-description needs `Foreign("eval")`, the core
is incomplete.

### Current state

The canonical `Language` (7 YAML files in `handl-kit-data-canonical/data/`)
describes handl's syntax and eval schemes as data. It is not yet
self-describing: the eval schemes for `call`, `var` lookup, and `object` key
resolution contain hardcoded Rust logic in the engines (acknowledged as debt
in ADR-0058). The arrow-calculus kernel (ADR-0052) is implemented and is the
sole `Scheme` vocabulary, but no `handl-defining-handl` data file exists, and
no proof exists that the kernel + desugar can cover handl's semantics without
`Foreign` escapes for meta-operations.

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

## Consequences

### Positive

- **Self-hosting becomes a proven property, not a claim.** The test either
  passes or it doesn't; no ambiguity about whether handl "really" describes
  itself.
- **The boundary between language and host is sharp.** `Foreign` means host
  effect, full stop. Language machinery lives in the core or desugar layer.
  This makes ADR-0046's capability discipline precise: a `Language` with no
  genuine host effects is provably safe to eval anywhere.
- **Downstream ADRs are unblocked.** Caching, chunking, and certificates
  ([0065](./0065-derived-artifact-cache-and-compression.md)–[0067](./0067-chunk-certificates.md))
  operate on a `Language` value that is complete and stable. There is no risk
  of optimizing a language that later turns out to need `Foreign` escapes and
  invalidates the cache/trust model.
- **`app` is the sole meta-operation.** `eval`/`compile` as host functions
  are eliminated; `app` (ADR-0052) is the calculus-native way to invoke an
  arrow received as data. This is the clean separation ADR-0052 §2
  specifies.

### Negative

- **The completeness proof may fail, and fixing it may require core
  extensions.** If a construct cannot be desugared to the current core
  without `Foreign`, the core must be extended (a versioned combinator,
  ADR-0044 §7) or the construct redesigned. This is real work with an
  uncertain scope — the ADR-0044 negative consequence acknowledged this.
- **Scope chain as arrow state is a non-trivial redesign.** `var` lookup as
  host function is the current implementation; expressing it as threaded
  arrow state changes the eval scheme structure for every binding form. This
  touches `eval.yaml` broadly.
- **The proof is only as good as the corpus.** A self-description that
  passes its own tests but misses a construct is a false positive. The
  corpus must cover every canonical surface construct, not a subset.

### Neutral

- **This ADR produces no new crates.** It is a property to prove and a
  constraint to enforce, not a subsystem to build. The artifacts are the
  `handl-defining-handl` data file, the extended conformance corpus, and the
  no-meta-`Foreign` assertion.

## Implementation phases

### Phase 1 — `Foreign` audit

Inventory every `Foreign` tag in `eval.yaml`. Classify each. Publish the
classification as a table in this ADR's successor or a research note. This is
investigation, not implementation.

### Phase 2 — Eliminate meta-`Foreign` escapes, one at a time

For each meta-operation identified in Phase 1, express it as arrow
combinators or a desugar rule. Migrate one at a time, with a parity test
against the existing Rust implementation (same pattern as ADR-0058's
object-key migration). Order: `call` dispatch (→ `app`), `var` lookup (→
threaded state), then any remaining escapes.

### Phase 3 — `handl-defining-handl` data file

Author the self-describing `Language` as a data file. Run it through the
extended conformance suite. The no-meta-`Foreign` assertion must pass.

### Phase 4 — Cross-engine conformance on self-description

Run the self-description corpus against tree-walk, bytecode (when available),
and cranelift (when available). Three-way conformance, per ADR-0044 Phase 8.

## References

- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as Data
  and Runtime Split (Phase 8 is the target; §"Negative consequences" is the
  risk this ADR addresses)
- [ADR-0052](./0052-arrow-calculus-kernel.md) — Arrow Calculus Kernel (`app`
  is the calculus-native meta-operation; `vau` removed)
- [ADR-0058](./0058-generic-data-driven-engines.md) — Generic Data-Driven
  Engines (engines are fixed interpreters; language logic migrates to data)
- [ADR-0046](./0046-capability-discipline-at-the-language-level.md) —
  Capability Discipline (`Foreign` means host effect; this ADR sharpens that
  definition)
- [ADR-0041](./0041-languages-as-first-class-values.md) — Languages as
  First-Class Values (the vision this ADR gates)
