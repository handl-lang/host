+++
id = "ADR-0053"
title = "ADR-0053: Generic Pratt Parser — Grammar Data Replaces Per-Construct Branches"
status = "open"
priority = "medium"
+++

---
title: Generic Pratt Parser — Grammar Data Replaces Per-Construct Branches
type: adr
date: 2026-09-02
status: proposed
status_note: Proposed — the five parser special cases and the literal heuristic are grammar data.
refs: [ADR-0044]
---

# ADR-0053: Generic Pratt Parser — Grammar Data Replaces Per-Construct Branches

## Decision

1. **`TokenKindDef.literal: Option<LiteralKind>`** replaces the
   `kind_id.contains(...)` heuristics in `atom_payload` with typed literal
   dispatch.
2. **`AstShapeRef.child_separators: Vec<ChildSeparator>`** replaces the
   `if/then/else` and `x = y in body` separator hardcoding.
3. **`DelimitedShape.open_is_operator_token: bool`** — the open delimiter is
   the prefix/infix token itself; replaces the `array`/`object`/`paren`/`call`
   hardcoding.
4. **`DelimitedShape.flatten: bool`** splices elements into the parent's
   children, keeping `call` flat for every eval engine.
5. **`DelimitedShape.rest_token: Option<String>`** replaces the
   `token.ellipsis` special case for rest/spread elements.
6. **The infix path consults `delimited`**, mirroring the prefix path — the
   hardcoded `call` branch disappears.
7. **Parser desugars move to a data-driven post-parse pass.** `if` becomes a
   real AST node (arity 3); the `if → match` rewrite is a transform
   (ADR-0094), not parser code.
8. **Contextual forms (`match`, `data`, `from`) are closed by grammar data** —
   per-child binding powers and trailing token steps, all `serde`-defaulted so
   third-party grammar rows are unaffected.

## Why

The Pratt engine still special-cased each construct — five branches plus a
substring heuristic for literals — so every new syntax meant engine code.
ADR-0044 (language as data) requires grammar rows to carry that knowledge; this
ADR removes the last per-construct branches and moves the parser's desugars to
the transform engine.

## Not doing

- **Turing-complete grammar description** — list comprehensions and
  multi-block constructs fall outside one `DelimitedShape` and stay parser
  code.
- **Keep desugars in the parser** — they move to the data-driven transform
  pass (ADR-0094).
- **Break third-party grammar rows** — every new field is `serde`-defaulted.

## Consequences

- `expression_inner` has no string literals; `atom_payload`,
  `parse_delimited_block` and `consume` are the only sub-routines.
- Grammar data grows and becomes the review surface; parser code shrinks.
- Contextual forms were closed by amendment without further engine changes.

## Enforcement

- `engine-parse-pratt/` — the generic parser crate.
- `kit-prelude-lang/crates/handl-kit-prelude-lang/data/grammar.yaml` — the
  rows that now carry the knowledge.
- Parser tests: `tests/` in `engine-parse-pratt`.

## Design

The five fields, before/after code shape, amendments and the scope boundary:
[`design/0053-generic-pratt-parser.md`](../design/0053-generic-pratt-parser.md).
