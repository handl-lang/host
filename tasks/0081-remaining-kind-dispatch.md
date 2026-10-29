---
title: 'ADR-0081 follow-up: the remaining kind-dispatch sites'
type: task
date: 2026-10-10
status_note: |-
  **proposal** (analysis complete; no code changed for these sites).
  Companion to [ADR-0081](../adrs/0081-data-driven-ast-child-roles.md).
---

# ADR-0081 follow-up: the remaining kind-dispatch sites

## What this closes, and what it doesn't

The structural-token pass (see `AGENTS.md` §"Engine-genericity residual
audit" (a2)) closed every `kind ==` site that could be expressed with
existing `Language` vocabulary:

- the Pratt engine's block opener / dimension bracket / block separators /
  keyword-as-identifier, via `token_kind_for_label` / `token_in_category` /
  `token_pair`;
- `desugar`'s `field-access` / `type-annot` reads, via `child_role_index`;
- `handl-runtime`'s identifier probes, via `token_in_category`;
- `plugin-lsp`'s duplicate `kind: export` lookups, via one helper.

What remains is **not** a sweep. Every residual site needs a *new*
`Language` field that does not exist yet, which is a language-schema
decision — adding vocabulary is how the invariant is extended, but it is
not a mechanical fix.

## The residual sites

### 1. `handl-kit-arrow-lower/src/lib.rs` — binding/arrow dispatch

`lower_block_child` and friends dispatch on `assign` / `bind` / `op`, and
`env_entry` / `function_params` on `function` / `rec` / `call` / `var`
(~11 sites).

The lowerer is already role-driven for child *positions*
(`child_at_role_or_pos(child, lang, "target", 0)`), so only the **kind
dispatch** is literal.

Why it can't be fixed today: there is no way to ask a `Language` "which
node kinds introduce a binding?" or "is this a lambda form?". The
canonical answer is a hand-written list of canonical kind names.

**Proposed vocabulary.** A face on `AstShapeDef`:

```yaml
- id: ast.assign
  node_kind: assign
  binding: true            # new
- id: ast.bind
  node_kind: bind
  binding: true            # new
```

with `Language::shape_has_face(kind, Face::Binding)`. The lowerer then
iterates the language's declared shapes instead of matching names. A
language that spells its binder `set` participates for free.

### 2. `host-cli/src/transform_categorical.rs` — `dim_arrow` analysis

Nine sites dispatch on `dim_arrow` / `stmt.import` / `var` / `value` /
`flow`. This is a *dimension-analysis* pass whose entire subject is the
`dim_arrow` shape, so the dispatch is narrower than it looks — but it is
still literal.

**Proposed vocabulary.** The pass needs to identify the arrow form and its
`dim`/`source`/`target` roles. `dim_arrow` already declares those roles, so
the fix is a `face` tag equivalent to (1) — e.g. `arrow: true` on the shape
— plus reading the roles it already has. `value` / `var` are literal-kind
detections already covered by `TokenKindDef::literal`, so those two can
likely use the existing literal path with no new vocabulary.

### 3. `plugin-ext-refinement/src/arrow_parser.rs` — **documented bootstrap**

23 sites (`arr`/`comp`/`first`/`choice`/`loop`/`app`/`id`/`flow`, plus the
hand-rolled import/export line scanner). ADR-0081's rule is that adding a
*calculus combinator* is an engine edit, and this vocabulary **is** the
calculus kernel rather than surface syntax. The line scanner must run
before arrow expressions parse, which is why it is not the Pratt engine.

**Recommendation: leave as bootstrap.** Do not chase these.

### 4. Smaller sites (decide case by case)

| Site | Nature | Likely verdict |
| --- | --- | --- |
| `platform/src/resolve.rs` (`kind == "call"`) | capability matching | identity-specific; leave |
| `plugin-engine-exec/src/pattern_match.rs` | role-less fallbacks | already `pattern_role`-driven for canonical shapes; leave |
| `handl-language/src/instantiate.rs` | bootstrap shape instantiation | names canonical kinds by construction; leave |
| `plugin-engine-typecheck-nbe/src/totality_check.rs` (`call`) | recursion detection | needs a `pattern_role: tag` for `call` patterns to close |

## Options

**Option A — add a `Face` enum to `AstShapeDef` (recommended).**
One new optional field, a small closed set of faces (`Binding`, `Arrow`,
`Call`, `Lambda`, …). Data-driven, one mechanism instead of four ad-hoc
tags, and the existing `pattern_role`/`literal_children` precedent shows
the shape. Cost: a schema addition plus a migration of ~20 call sites.

**Option B — per-feature booleans (`binding: true`, `arrow: true`, …).**
Cheaper to read, but open-ended: each new need adds a field, and there is
no way to ask "what faces does this kind have?".

**Option C — leave everything as-is and document.**
Defensible for (3) and (4). Not defensible for (1): `handl-kit-arrow-lower`
is a *generic* lowerer, and a third-party kit with renamed binder kinds
cannot use it — the exact breach ADR-0081 exists to prevent.

## Recommendation

1. Do **Option A** (`Face`) for (1) and (2) — the two sites that are
   genuinely generic components.
2. Explicitly document (3) and (4) as accepted bootstrap, so the next
   audit does not re-open them (they are already listed in `AGENTS.md`).
3. Add a cross-language acceptance test in the ADR-0081 Phase 4 style: a
   language whose binder kind is renamed (`set` instead of `assign`) must
   lower and evaluate unchanged. Without that test, "the lowerer is
   generic" is an untested claim.

## Prior art in this repo

- `AstShapeDef::pattern_role` (ADR-0081) — a closed vocabulary
  (`sequence`/`entries`/`entry`/`rest`/`leaf`) that engines dispatch on
  instead of `kind ==`. A `Face` field is the same idea for
  binding/arrow/call/lambda detection.
- `AstShapeDef::literal_children` — role → child-kind list; already the
  mechanism that makes literal-vs-computed keys data-driven.
