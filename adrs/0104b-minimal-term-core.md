---
title: Minimal `Term` Core with Generic Combinators
type: adr
date: 2026-09-09
status: proposed
status_note: Proposed
---

# ADR-0104b: Minimal `Term` Core with Generic Combinators

## Context

The `Term` kernel currently exposes a large number of concrete frontier
forms (`CellCompH`, `Transport`, `Cobox`, `Witness`) that are not actually
primitive: they only package structural or record data that can be built
from a smaller set of generic combinators. Keeping them as first-class
kernel variants forces the metalang `ValueModel` to maintain a broad
dispatch table and complicates the eval/type rules. We need a smaller,
more compositional `Term` core.

## Decision

Refine the `Term` kernel to a minimal core by adding five generic
combinators and desugaring the four concrete frontier forms listed below
into that core. `eval` will call `Term::desugar(&self) -> Term` once
before evaluation, and the canonical `eval.yaml` rules will be updated to
emit core terms directly.

## Core `Term` combinators to add

```rust
Const { value: serde_json::Value }
Src  { cell: Term }
Tgt  { cell: Term }
RecordProj { record: Term, field: String }
RecordWith { record: Term, field: String, value: Term }
Tag  { tag: String, value: Term }
```

- `Const { value }` — a constant arrow that ignores its incoming value
  and returns `value`.
- `Src { cell }` / `Tgt { cell }` — extract the source or target 1-cell of
  a 2-cell `cell`.
- `RecordProj { record, field }` — project `field` out of a record value.
- `RecordWith { record, field, value }` — return `record` with `field`
  set to `value`.
- `Tag { tag, value }` — shorthand for `RecordWith` with `__tag__` set to
  `tag` and the remaining fields taken from `value`.

## `Term` variants to remove

The following concrete forms are desugared into the core and removed from
the `Term` enum:

- `CellCompH`
- `Transport`
- `Cobox`
- `Witness`

The following variants remain primitive and are not touched:

`CellId`, `CellCompV`, `Whisker`, `Path`, `Equiv`, `Univalence`, `Glue`,
`Fibrant`, `JElim`, `Cobind`, `Refine`, `Iter`, `Loop`, `Arr`, `Op`,
`Seq`, `First`, `Choice`, `App`, `Handle`, `Child`, `Rewrite`.

## Desugaring rules

`Term::desugar` rewrites each removed form as shown below. Desugaring is
recursive over child terms.

### `CellCompH`

```text
CellCompH { left, right }
  ->
CellCompV {
  first: Whisker { side: Right, arrow: Tgt(right), cell: left },
  second: Whisker { side: Left, arrow: Src(left), cell: right }
}
```

This is the standard 2-category horizontal composition formula.

### `Transport`

```text
Transport { path, value }
  ->
JElim { path, motive: Op("const", [Tgt(path)]), base: value }
```

If `Tgt(path)` cannot be represented as a `Value`, `Transport` may instead
be encoded as:

```text
Transport { path, value } -> Op("transport", [path, value])
```

The first encoding is preferred when path endpoints are `Value`s.

### `Cobox`

```text
Cobox { cell }
  ->
RecordProj { record: cell, field: "value" }
```

The runtime stores the raw cobox value under a `value` field.

### `Witness`

```text
Witness { grade, proof }
  ->
RecordWith {
  record: Tag {
    tag: "Witness",
    value: RecordWith { record: Const(Null), field: "type", value: proof }
  },
  field: "grade",
  value: grade
}
```

## Runtime semantics

- Each new core combinator receives one `ValueModel` rule and one
  `TypeModel` rule in the metalang runtime.
- `Const` always returns its payload; typing is the literal JSON schema of
  the payload.
- `Src`/`Tgt` require 2-cell values to carry source and target 1-cell
  metadata; they project that metadata.
- `RecordProj`, `RecordWith`, and `Tag` operate on JSON-like record
  values.
- Desugared forms do not get their own rules; they inherit the rules of
  the core combinators they expand into.

## Migration

1. Implement `Term::desugar(&self) -> Term` as a recursive helper that
   applies the rules above.
2. Update all three eval engines so that `eval` calls `t.desugar()` once
   before interpreting a term.
3. Update `eval.yaml` rules to emit core terms directly; keep the
   desugaring helper for backward compatibility and for terms supplied by
   external tools.
4. Remove the removed variants from the `Term` Rust enum and from any
   hand-written matches. Tests that construct `CellCompH`, `Transport`,
   `Cobox`, or `Witness` directly must use the equivalent desugared form
   or call `desugar()`.

## Risks

- `Src` and `Tgt` require 2-cell values to explicitly store source and
  target 1-cells. If the runtime does not currently attach this metadata,
  the value model for 2-cells must be extended.
- `Const` must hold a full `serde_json::Value`. This means `Transport`
  can only desugar to `JElim` with a `const` motive when `Tgt(path)` is a
  `Value`; otherwise the `Op("transport", ...)` encoding must remain as a
  primitive op.
