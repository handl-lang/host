---
title: '`TransformRule::Replace` — a rule form for root-eliminating rewrites'
type: adr
date: 2026-10-15
status: accepted
---

# ADR-0127: `TransformRule::Replace` — a rule form for root-eliminating rewrites

## Context

A transform rule describes a pure `Ast → Ast` rewrite dispatched on
`(pass, node_kind, priority)` (ADR-0094). Until now a rule had exactly two
forms:

- `Declarative { target_kind, children, payload }` — build a **new** node of
  `target_kind` from child steps; and
- `Native(name)` — hand the rewrite to a wired native-rewrite registry.

`Declarative` **always allocates a node**. That is fine for the sugar
lowerings it was designed for (`while`, `nullish`, …), but it cannot express
a rewrite that *drops its root* and returns an existing subterm — a
**root-eliminating** rewrite. Such rewrites are not exotic: they are the
identity laws of any algebraic bank (`neg(neg(a)) → a`, `a + 0 → a`,
`transpose(u, d, d) → u`, `chunk(u, chunks=1) → u`).

This was not theoretical. catopt's law bank — emitted as handl data and
loaded through `language_from_dir` — contains **five** such laws, and the
integration could not represent them. The failure was measured, not assumed:

- the producer's first attempt emitted `target_kind: None`. Because
  `TransformRule::Declarative.target_kind` is a Rust `String` and
  `language_from_dir` reads `transforms.yaml` with
  `serde_yaml::from_str::<Vec<TransformDef>>`, serde_yaml coerced the YAML
  `null` scalar into the **string `"null"`**. The row loaded, and
  `walk.rs::fire_untraced` → `step.rs::build_rewrite` (`kind:
  target_kind.to_string()`) **silently fabricated a node of kind `"null"`**
  whose children were the kept child steps. `validate_transforms` only
  *warns* about the unknown kind, so assembly still succeeded. A silent lie,
  not a refusal.
- the producer then emitted an explicit refusal — `Native("catopt:eliminate_root")`
  — which errors loudly (no registry is wired) but still cannot *express*
  the law.

No existing vocabulary closes the gap. `RewriteStep::Splice(i)` lifts a
child's *children* into the created node — the `Declarative` node still
exists. `RewriteStep::KeepPath(path)` navigates a path and produces a
*child*, never the rewritten node itself. `Native` *can* return an existing
node (`RewriteContext::remap_child` yields an `AstRef`), but the only
implementations available for these five laws would be the *producer's* Rust
running inside handl's engine — which makes the differential test between
two independent engines circular for exactly the laws it most needs to check.

## Decision

Add one variant to `TransformRule`, a **calculus-vocabulary** addition (the
kind of change ADR-0081 permits in Rust):

```rust
Replace { path: Vec<usize> },
```

Semantics: **the rewritten node IS the node at child-index `path` below the
matched node, remapped into the output arena.** No node is created.

- `path` is interpreted exactly as `RewriteStep::KeepPath`'s — a non-empty
  sequence of child indices starting at the matched node. `Replace { path:
  [0, 0] }` on `neg(neg(a))` is precisely `double_neg`.
- A `path` that does not resolve to an existing descendant (out of range, or
  empty) is a `LangError`, **never** a silently fabricated node. This is the
  property the `"null"` bug lacked.
- The rule stays data-shaped: dispatch is still `(pass, node_kind,
  priority)`, and the engine change is **one arm** in
  `walk.rs::fire_untraced` plus a validation arm. The path navigation is
  shared with `KeepPath` (`step.rs::descendant_index`), so `Replace` is
  literally "a `KeepPath` promoted to the rule level".

### What it deliberately does not do

- It does **not** create, splice, or wrap anything. `Replace` returns a node
  that already exists in the input subtree; a rewrite that needs a *new*
  node still uses `Declarative` or `Create`.
- It does **not** change `KeepPath`'s existing behaviour (a missed path in a
  child step still yields a `Null` child — the vocabulary's long-standing
  "no node here" spelling). Only the *rule-level* form is strict, because a
  rule must return a node.
- It is **not** a general subterm-selection language. There is one path, from
  the matched node; no expressions, no conditions on the target beyond
  resolution.
- It does **not** make `Native` redundant. `Native` remains the escape hatch
  for rewrites that need computation; `Replace` covers the declarative
  subset that navigation alone can express.

## Consequences

- A root-eliminating law is now expressible as data. catopt's bridge emits
  `rule: !Replace { path: [...] }` for a bare-metavar right-hand side bound
  in the left-hand side; the five bank laws (`id_add`, `id_mul`,
  `double_neg`, `transpose_noop`, `chunk_single`) leave the
  "unrepresentable" bucket and become **comparable** in the cross-engine
  differential (`lab/audit/handl_differential.py` in the catopt tree).
- The failure mode that motivated the ADR — a rule that cannot be expressed
  degrading into a *fabricated* node — is closed at the engine: an
  unresolvable path is an error, and `validate_transforms` errors on an
  empty path or an out-of-range first index (mirroring `KeepPath`'s
  first-step check).
- The variant is externally tagged like every other rule form, so it is
  `!Replace` in YAML and `{"Replace": {"path": [...]}}` in JSON.
- Any consumer that matches exhaustively on `TransformRule` must handle the
  new arm; in-tree that is the rewrite engine and `validate_transforms`, both
  updated here.
- Cost: one enum variant, one engine arm, one validation arm, one shared
  navigation helper. No new runtime dependency, no schema version bump (the
  vocabulary is additive and the enum was already open-ended by design).

## Status

accepted
