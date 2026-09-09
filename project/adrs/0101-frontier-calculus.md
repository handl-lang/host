# ADR-0101: Frontier Calculus `Term` Variants

## Status

Accepted / implemented.

## Implementation status

The frontier-calculus primitives are wired into the `Term` arrow-calculus
kernel and the `Side` helper enum. The variants and the enum are derived
`Serialize`/`Deserialize` and first-order serializable data.

Implemented:

- `Term::CellId`, `Term::CellCompV`, `Term::CellCompH`, `Term::Whisker`,
  `Term::Univalence`, `Term::Glue`, `Term::Fibrant`, `Term::JElim`,
  `Term::Witness`, `Term::Refine`, `Term::Cobind`, `Term::Cobox`.
- `Side` enum (`Left`, `Right`) for `Whisker` attachments.

Partial:

- `CellId`, `CellCompV`, `Whisker`, `Witness`, `Refine`, `Cobind`,
  `Cobox`, and `Univalence` have `ValueModel` and `TypeModel` semantics.
- `CellCompH`, `Glue`, `Fibrant`, and `JElim` are present in the `Term`
  data but currently return `NotImplemented` in the value engine and
  pass through in the type engine.

## Context

ADR-0100 establishes directed homotopy type theory as the long-term
theoretical limit for the platform. Its higher cells, paths, univalence,
fibrations, and refinement/witness machinery are the *concepts*.

This ADR records the concrete, accepted `Term` data constructors that
carry those concepts in the arrow-calculus kernel. It does not restate the
directed-HoTT theory; see ADR-0100 for the 2-categorical and model framing.

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

## Consequences

### Positive

- Gives the directed-HoTT concepts from ADR-0100 a concrete,
  serializable `Term` representation.
- Keeps the frontier layer data-driven: no new Rust closures or
  engine-specific value types are required.
- Adds refinement, witness, and cobind support for grades/refinement types
  without changing the existing 1-cell kernel.
- Provides a staging ground for `CellCompH`, `Glue`, `Fibrant`, and `JElim`
  implementations in future passes.

### Negative

- Four variants (`CellCompH`, `Glue`, `Fibrant`, `JElim`) are currently
  unimplemented in the value engine.
- The call-style surface syntax is verbose; an operator pass is still
  required for ergonomic use.
- `ValueModel` and `TypeModel` semantics for the implemented variants are
  deliberately shallow in this pass; richer HoTT semantics will increase
  model complexity later.
