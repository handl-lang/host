# ADR-0123: AST shape faces — closing the last kind-dispatch sites

## Context

[ADR-0081](./0081-data-driven-ast-child-roles.md) gave `AstShapeDef` **child
roles**, so an engine can ask "what is child 0 of this node *for*?" instead of
assuming a position. That closed the positional half of the engine-genericity
problem.

The structural-token pass then closed every site expressible with *existing*
vocabulary — the Pratt engine's block opener, dimension bracket, block
separators and keyword-as-identifier resolve through
`Language::token_kind_for_label` / `token_in_category` / `token_pair`; the
`desugar` natives resolve `field-access`/`type-annot` children by role;
`handl-runtime` classifies identifiers by token category. See `AGENTS.md`
§"Engine-genericity residual audit" (a2).

What is left is a different shape of hardcoding. These sites do not guess a
child position — they **dispatch on the canonical kind name**:

| Site | Dispatch |
| --- | --- |
| `handl-kit-arrow-lower` | `assign` / `bind` / `op` / `function` / `rec` / `var` / `call` |
| `host-cli/src/transform_categorical.rs` | `dim_arrow` / `stmt.import` / `var` / `value` / `flow` |
| `plugin-engine-typecheck-nbe/src/totality_check.rs` | `call` patterns |

Roughly 20 sites in total. Child *positions* are already role-resolved at
these sites (`child_at_role_or_pos(child, lang, "target", 0)`); only the
kind check is literal.

The concrete cost: `handl-kit-arrow-lower` is a **generic** component, but a
language that spells its binder `set` instead of `assign` cannot use it
without an engine edit. That is precisely the breach ADR-0081 and
[ADR-0044](./0044-language-as-data-and-runtime-split.md) exist to prevent.
There is no way to ask a `Language` "which node kinds introduce a binding?",
"which shape is the arrow form?", or "is this a call pattern?".

`plugin-ext-refinement/src/arrow_parser.rs` is **out of scope**: its
vocabulary (`arr`/`comp`/`first`/`choice`/`loop`/`app`/`id`/`flow`) is
arrow-calculus kernel syntax, not surface syntax, and ADR-0081 already rules
that adding a calculus combinator is an engine edit.

## Decision

Add an optional **`faces`** set to `AstShapeDef` — a small, closed vocabulary
of structural *roles a node kind plays* — and a `Language::shape_has_face`
lookup. Engines consume faces instead of kind names.

```yaml
- id: ast.assign
  node_kind: assign
  faces: [binding]
- id: ast.bind
  node_kind: bind
  faces: [binding]
- id: ast.function
  node_kind: function
  faces: [lambda]
- id: ast.call
  node_kind: call
  faces: [call]
- id: ast.dim_arrow
  node_kind: dim_arrow
  faces: [arrow]
```

The initial vocabulary is **not** simply `binding` — see "Finding" below —
but a set that distinguishes the structural layouts engines actually branch
on. Provisional:

```yaml
- id: ast.assign
  node_kind: assign
  faces: [binding, binding_target_value]
- id: ast.bind
  node_kind: bind
  faces: [binding, binding_named]
```

## Finding: `binding` alone is not sufficient

A first implementation attempt (reverted) added a single `binding` face and
migrated `handl-kit-arrow-lower`'s block-child dispatch. It failed for a
structural reason worth recording:

`assign` and `bind` are **two different binder layouts**, not two spellings
of one:

| Kind | Layout |
| --- | --- |
| `assign` | children `target` / `value` (roles) |
| `bind` | `payload.name` + a single value child |

The lowerer branches on which layout it is looking at, so a single
`binding` face cannot select the branch — a renamed `assign` would be
treated as a `bind`, or vice versa. The attempted fix (`if is_binding(kind)
&& kind != "bind"`) preserved canonical behaviour but delivered **zero**
genericity while adding indirection, i.e. strictly worse than the literal
check it replaced.

**Conclusion:** a face must identify the layout, not just the family. Either
declare the layout faces as above, or rely on the existing child-role
metadata (`target`/`value` roles vs a `name` payload) plus a `binding` face
for the family. The second is preferable — it needs no new layout vocabulary
and uses `child_role_index`, which already exists — and should be tried
first on the next attempt.

This is the reason the ADR is `proposed` rather than `accepted`: the
vocabulary is not yet validated by an implementation.


A lookup method mirrors the existing shape helpers:

```rust
pub fn shape_has_face(&self, node_kind: &str, face: Face) -> bool;
```

Callers iterate the language's declared shapes (or test the current node)
rather than matching on names, and keep their positional fallback for
role-less shapes exactly as they do today for `literal_child` /
`pattern_role`.

## Consequences

**Positive.**

- `handl-kit-arrow-lower` becomes usable by a language with renamed binder
  kinds — the actual goal.
- One mechanism replaces the ad-hoc alternatives; the vocabulary is closed
  and enumerable, like `pattern_role` (`sequence`/`entries`/`entry`/`rest`/
  `leaf`).
- The remaining ~20 sites become a mechanical migration rather than a series
  of judgement calls.

**Negative / costs.**

- A schema addition to `AstShapeDef` plus migration of ~20 call sites and
  their tests.
- Faces can be mis-declared. Mitigation: an acceptance test in the ADR-0081
  Phase 4 style — a language whose binder kind is renamed (`set` for
  `assign`) must lower and evaluate unchanged. Without that test, "the
  lowerer is generic" stays an untested claim.
- A face is *additive* metadata; a shape with no faces behaves exactly as
  today, so third-party kits are unaffected.

**Rejected alternative: per-feature booleans (`binding: true`,
`arrow: true`, …).** Cheaper to read, but open-ended — each new need adds a
field, and there is no way to ask "what faces does this kind have?" or to
exhaust the vocabulary in a match.

**Rejected alternative: leave the sites and document them.** Defensible for
`transform_categorical` (a `dim_arrow` analysis pass) and the `call` pattern
check, but not for `handl-kit-arrow-lower`, which is a generic component.

## Status

**Proposed.** Not implemented. The site-by-site analysis and the migration
inventory live in
[`project/tasks/0081-remaining-kind-dispatch.md`](../tasks/0081-remaining-kind-dispatch.md).
