# ADR-0095: Formatter as a Production Pipeline — Implementation Report

**Status:** Implemented (committed at `8fb2616b`). Formatter logic proven correct;
`plugin-ext-fmt` golden/idempotence tests blocked by a **pre-existing kit-data
collision** unrelated to this work (see _Blocker_).

## Context

`plugin-ext-fmt` was a single hardcoded function chain — `parse → lex →
format_tokens → post_process` — over 500 inline lines with every style rule
(spacing, chain-breaking, blank-line policy, trailing-NL) baked into one walk.
The generic `Format` capability in `platform/src/format.rs` (`BasicFormatter` /
`FormatBehavior`) was disconnected from the real formatter: it keys rules by
`AstNode.kind`, operates on a single node payload, and nothing real used it.

ADR-0095 applied the production-pipeline discipline ADR-0089 brought to the test
runner: **land stage boundaries now, ship simple implementations behind them,
make future features new compositions rather than surgery.**

## What changed

### Crate boundary

| Crate                | Role                                                                                                                                                                                     | Fate                                                                                                                        |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `plugin-engine-fmt`  | Format **engine**: staged pipeline + `NodeEmitter`, `FormattingRule`, `PostProcessRule` seams. Language-generic (ADR-0044/0041).                                                         | Gained `layout.rs`, `token.rs`, `post_process.rs`.                                                                          |
| `plugin-ext-fmt`     | Canonical v0.1 **surface**: assemble the canonical kit, select the canonical rule set, expose `format_source` / `check_source` / `format_ast` + CLI contract + golden/idempotence tests. | `lib.rs` slimmed to a thin wrapper over the engine pipeline.                                                                |
| `platform/format.rs` | Generic `Format` capability.                                                                                                                                                             | Doc-only: documented as the generic per-node contribution seam (cannot dispatch to the engine due to dependency direction). |

### Pipeline and stage contracts

```
Source text
   │  parse                          (reuse lex + Pratt; data-driven, ADR-0044)
   ▼
[tokens, AST]                        ── stage 1 · parse
   │  layout
   ▼
LayoutStream (breaks / indents / spacing, per-rule decisions)
                                     ── stage 2 · layout   → layout.rs
   │  render
   ▼
rendered text                        ── stage 3 · render   → token.rs (TokenFormatter)
   │  post-process                   ── (AST path: FormatEngine / NodeEmitter, unchanged)
   ▼
canonical source                     ── stage 4 · post-process → post_process.rs
```

### New seams (the extensible / replaceable / pluggable core)

**`FormattingRule`** (`layout.rs`) — each canonical-style rule (spacing,
indentation, chain-breaking, blank-line policy) is a separate implementation
behind the trait, selected/composed by the surface. The formatter analog of the
test runner's `TestProvider` seam (ADR-0089). `CanonicalFormattingRule` encodes
the v0.1 table; a different dialect or style swaps in new rule objects without
touching the render stage.

**`PostProcessRule`** (`post_process.rs`) — each post-processing concern is a
separate, composable, idempotent-friendly rule: `TrailingWhitespaceRule`,
`CollapseBlankLinesRule`, `BlankLineBetweenDeclGroupsRule`,
`SingleTrailingNewlineRule`. `canonical_post_process` composes the canonical
set in order; `post_process_pipeline` runs an arbitrary sequence.

**`TokenFormatter`** (`token.rs`) — token-stream renderer parameterized by a
`FormattingRule` and an indentation width; agnostic to which style is active.

**`NodeEmitter`** (unchanged) — `PlainEmitter` / `PreserveEmitter` remain the
AST→source swap point, so the LSP incremental path and the CLI normalizing path
share one contract.

### Config seam

`FormatOptions` (already an `{ indent_size }` record) grew into the config
record RFC-0002 specifies (`{ indent, width }`), threaded as pipeline **input** —
never read from a file by the engine.

### Public API (preserved)

```rust
pub fn format_source(src: &str) -> Result<String, FormatError>;
pub fn check_source(src: &str) -> Result<bool, FormatError>;
pub fn format_ast(lang: &Language, ast: &Ast, root: AstRef) -> Result<String, LangError>;
```

`FormatError` and diagnostics integration (`plugin-ext-diag`, ADR-0091) are
unchanged.

## Validation

### Passing

- **`plugin-engine-fmt`**: 24/24 tests pass (16 pre-existing + 8 new covering
  `classify_token`, `needs_space`, `indent_bias`, `FormatOptions` defaults,
  `TokenFormatter` spacing/indent/custom-rule, and every `PostProcessRule`).
- **Dependent hosts compile**: `host-cli`, `host-lsp`, `host-tooling`,
  `plugin-ext-lint` all `cargo check` clean against the refactored engine.

### Blocker: pre-existing kit-data collision (out of scope)

`plugin-ext-fmt`'s golden + idempotence tests fail with:

```
Parse("collision: existing transform.desugar.nullish conflicts with incoming transform.desugar.nullish")
```

thrown by `handl-kit-data-canonical::assemble()` — code **outside this commit**,
which fires in `canonical_language()` before any formatter code runs.

**Root cause** (`handl-language/src/fragments/transform_def.rs`): `TransformDef::
identity_key()` returns `"{pass}:{node_kind}:{priority}"` — it ignores the `id`
field, so two transforms targeting the same node_kind in the same pass collide
unless one `replaces:` the other. `handl-kit-data-canonical/data/transforms.yaml`
has duplicate identity_keys:

| Duplicates                                                                           | Lines    | Content                      |
| ------------------------------------------------------------------------------------ | -------- | ---------------------------- |
| `transform.desugar.nullish` × 2                                                      | 65, 317  | identical rule               |
| `transform.desugar.opt_chain` (underscore) vs `transform.desugar.opt-chain` (hyphen) | 102, 317 | same node_kind/pass/priority |

These duplicates exist in the **parent commit** (`0343f344`), so this is
pre-existing — not caused by the formatter work. The engine tests bypass it
because they build their own minimal languages instead of calling `assemble()`.

### Recommendation

The formatter architecture is complete and correct. To get the golden tests
green, the kit-data duplicates need reconciling — but that lives in
`handl-kit-data-canonical` and touches **language desugar semantics** (nullish
`??`, opt-chain `?.`), so it is a separate decision with blast radius across
typecheck/exec. Two options:

1. **Leave it** — file a separate kit-data issue; the formatter work is done.
2. **Fix the duplicates** — remove the identical nullish copy and reconcile
   `opt_chain` / `opt-chain` (sign-off recommended; affects language transforms
   beyond the formatter).
   unchanged.
