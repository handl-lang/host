---
title: Frontier Calculus `Term` Variants — design detail
type: design
date: 2026-09-09
updated: 2026-10-29
refs: [ADR-0202]
---

> Imported from `kit-data-prelude/project/design/0101-frontier-calculus.md` — ADR-0101 renumbered to ADR-0202 in the host merge.

# ADR-0202: Frontier Calculus `Term` Variants — design detail

Moved verbatim from ADR-0202 (ADR-0161 size budget). The ADR carries the rules;
this page carries the mechanism, tables and rationale.

## Decision

### 1. New `Term` variants

The `Term` enum gains twelve frontier-calculus constructors:

```rust
pub enum Term {
    // ... existing variants (Arr, Op, Seq, First, Choice, Loop, App,
    // Iter, Child, Handle, Rewrite, Path, Equiv, Transport) ...

    /// Identity of a cell in a frontier context.
    CellId { arrow: Box<Term> },

    /// Vertical cell composition.
    CellCompV { first: Box<Term>, second: Box<Term> },

    /// Horizontal cell composition.
    CellCompH { left: Box<Term>, right: Box<Term> },

    /// A whisker attaching an arrow to a cell on a side.
    Whisker { side: Side, arrow: Box<Term>, cell: Box<Term> },

    /// Univalence from an equivalence.
    Univalence { equiv: Box<Term> },

    /// Glue a family along a path.
    Glue { family: Box<Term>, path: Box<Term> },

    /// Fibrant type former.
    Fibrant { base: Box<Term>, family: Box<Term> },

    /// J-elimination.
    JElim { path: Box<Term>, motive: Box<Term>, base: Box<Term> },

    /// Witness carrying grade metadata and a proof.
    Witness { grade: Box<Term>, proof: Box<Term> },

    /// Refine a sort by a predicate.
    Refine { sort: Box<Term>, predicate: Box<Term> },

    /// Cobind over a grade and body.
    Cobind { grade: Box<Term>, body: Box<Term> },

    /// A cell box.
    Cobox { cell: Box<Term> },
}

/// A side for a `Whisker` cell attachment.
pub enum Side {
    Left,
    Right,
}
```

All new variants are pure structural data: `Box<Term>` pointers to
sub-schemes, strings, and the `Side` enum. None contain closures,
function pointers, or engine-specific state.

### 2. Surface syntax

The variants are currently exposed as **call-style forms** in the
frontier layer:

```handl
CellId(arrow)
CellCompV(first, second)
CellCompH(left, right)
Whisker(Left, arrow, cell)
Univalence(equiv)
Glue(family, path)
Fibrant(base, family)
JElim(path, motive, base)
Witness(grade, proof)
Refine(sort, predicate)
Cobind(grade, body)
Cobox(cell)
```

`Left` and `Right` are the two constructors of `Side`.

This is a conservative first pass: every frontier constructor is a named
call with positional arguments, consistent with how `op` and other
calculus forms are parsed. An **Option B operator pass** is planned to add
more compact surface syntax (e.g. infix vertical/horizontal composition
operators and dedicated path/univalence tokens) once the semantics are
green in the value and type models. That pass is now documented below.

### 2a. Surface syntax (Option B)

The approved compact surface forms are:

- `id2 f` (cell_id)
- `alpha =>> beta` (cell_comp_v)
- `alpha =>| beta` (cell_comp_h)
- `h <@ alpha` (whisker_left)
- `h @> alpha` (whisker_right)
- `p # v` (transport)
- `A where P` (refine)
- `v as G` (witness)
- `G ?> body` (cobind)
- `ua(e)` (univalence)
- `glue(family, path)` (glue)
- `fibrant(base, family)` (fibrant)
- `J(path, motive, base)` (JElim)
- `cobox(c)` (cobox)

> **Implementation status:** Call-style prefix forms were the first pass; Option B operators are the canonical surface.

### 3. Engine semantics

`ValueModel` (tree-walk and compiled engines):

- `CellId` evaluates `arrow` and returns the result.
- `CellCompV` evaluates `first`, then `second` with the first result as
  the incoming value.
- `Whisker` reorders `cell` and `arrow` based on `Side` and evaluates them
  sequentially, feeding the first result into the second.
- `Witness` ignores `grade` and evaluates `proof`.
- `Refine` evaluates the predicate on the incoming value; if truthy it
  returns the incoming value, otherwise it raises a runtime error.
- `Cobind` ignores `grade` and evaluates `body`.
- `Cobox` evaluates the wrapped `cell`.
- `Univalence`, when the incoming value is an `Equiv` tagged object,
  re-tags it as `Path` carrying the same `a`, `b`, and `path` fields.
- `CellCompH`, `Glue`, `Fibrant`, and `JElim` currently return
  `NotImplemented`.

`TypeModel`:

- `CellId`, `Witness`, `Refine`, `Cobind`, and `Cobox` evaluate their
  primary sub-scheme and return its type.
- `Univalence` evaluates the equivalence and returns `type.bool`.
- `CellCompH`, `Glue`, `Fibrant`, and `JElim` are stubs that return the
  incoming type or `Null`.

This matches the policy in ADR-0100: the full HoTT theory is staged; the
`Term` data accepts the complete vocabulary, and individual models
implement only the cells they need for the current pipeline.

### 4. Serialization

All twelve new variants and `Side` derive `serde::Serialize` and
`serde::Deserialize`. They are first-order serializable data: every field
is either a primitive (`String`, `Side`), an `Option`, or a recursively
nested `Box<Term>`. There are no closures, function pointers, or
engine-local handles, so the frontier `Term` language round-trips through
JSON and the language data assembly unchanged.

### 5. Relation to existing ADRs

- ADR-0100 provides the directed-HoTT limit and the `Model` trait design.
  This ADR is the concrete `Term` data step for that theory.
- ADR-0052 defines the arrow-calculus kernel; these constructors extend it.
- ADR-0094 explains the `Term` 2-category and rewrite engine; frontier
  cells are additional 2-cells and fibrancy/witness gadgets in that same
  data model.

## Why

ADR-0100 establishes directed homotopy type theory as the long-term theoretical limit for the platform. Its higher cells, paths, univalence, fibrations, and refinement/witness machinery are the _concepts_.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- Gives the directed-HoTT concepts from ADR-0100 a concrete,
- Keeps the frontier layer data-driven: no new Rust closures or
- Adds refinement, witness, and cobind support for grades/refinement types
- Provides a staging ground for `CellCompH`, `Glue`, `Fibrant`, and `JElim`
- Four variants (`CellCompH`, `Glue`, `Fibrant`, `JElim`) are currently

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0202-frontier-calculus.md`](../design/0202-frontier-calculus.md).
