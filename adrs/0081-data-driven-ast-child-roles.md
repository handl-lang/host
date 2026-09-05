# ADR-0081: Data-Driven AST Child Roles — Closing the Last Engine Hardcoding

## Status

Accepted. Phases 1–4 are implemented and the Phase 4 cross-language acceptance test passes (renamed kinds `set`/`block2`/`ret`/`params2`/`func2` with identical roles run through unchanged tree-walk and bytecode engines). Phase 5 (v1 deletion) remains coordinated with the [ADR-0052](./0052-arrow-calculus-kernel.md) cutover and is not blocking acceptance — the v2 engines are role-aware and the v1 path is no longer extended.

## Context

ADR-0058 (Accepted) migrated four primitives (`var`/`call`/`object`/pattern-match) to data and declared "no new language-specific logic in engines." That claim holds for those four. But `eval_block`, `closure`, and typecheck shape dispatch still hardcode `kind ==` checks ADR-0058 didn't cover:

| Engine | Site | Hardcoded check |
|---|---|---|
| `plugin-engine-eval-tree-walk` | `closure` | `kind == "paren" \|\| "array"` (1053) |
| `plugin-engine-eval-tree-walk` | `closure` key | `kind == "var" \|\| "string"` (1249, 1259) |
| `plugin-engine-eval-tree-walk` | `eval_block` | `kind == "assign"` (1544, 2283, 2352) |
| `plugin-engine-eval-tree-walk` | `eval_block` | `kind == "return"` (2323) |
| `plugin-engine-typecheck-nbe` | synth/check | `kind == "type-annot"` (884, 1109) |
| `plugin-engine-typecheck-nbe` | block | `kind == "block"` (1247) |
| `plugin-engine-typecheck-nbe` | params | `kind == "paren" \|\| "var"` (1378, 1403, 3593) |
| `plugin-engine-typecheck-nbe` | block children | `kind == "assign" \|\| "return"` (2628, 2631, 2695) |

A primitive that needs "the binding target of an `assign`" can only get it by testing `kind == "assign"` and assuming child 0 is the target. A language with different kind names (`set` instead of `assign`, `params` instead of `paren`) can't reuse these primitives without engine edits — a breach of "language as data" ([ADR-0044](./0044-language-as-data-and-runtime-split.md)).

`AstShapeDef` has no child-role mechanism — children are referenced purely positionally via `proj_child(index)`. The role metadata lives only in engine Rust.

The closed native-op set (`add`/`sub`/`proj_child`/...) and the six kernel combinators are **not** violations — they're the engine's interpreter vocabulary. This ADR adds zero of either.

## Decision

Add **named child roles** to `AstShapeDef`; resolve them to positional indices at assembly time. Engine, native-op set, and kernel combinators stay unchanged.

### 1. `AstShapeDef` gains optional `children`

```yaml
- id: ast.assign
  node_kind: assign
  children:
    - role: target
    - role: value
- id: ast.block
  node_kind: block
  children:
    - role: statements
      variadic: true
```

`children: Option<Vec<ChildRoleSpec>>`. Absent = opaque, positional fallback (backward compatible). `variadic: true` resolves to `[start_index..]`.

### 2. `proj_child` accepts a role name

```
arr_native_lit("proj_child", [], 0u64)        # positional (existing)
arr_native_lit("proj_child", [], "target")    # by role (new)
```

The `lit` payload is already `Value`; accepting `String` is a payload-shape extension, not a new op.

### 3. Resolve at assembly time (preferred)

`handl-kit-data-canonical` rewrites `proj_child("role")` → `proj_child(index)` by looking up the role in `AstShapeDef`. The runtime engine sees only positional `proj_child(u64)` — zero engine changes. (Fallback: runtime resolution in the `proj_child` handler if assembly-time proves awkward. Settled in Phase 1.)

### 4. Rewrite the violating primitives

Replace `kind ==` checks with role lookups. `eval_block` uses `proj_child("statements")`; each statement's own eval scheme handles binding vs return. `closure` uses `proj_child("params")`. Typecheck uses `synth_child("role")` / `check_child("role")`.

### 5. Declare roles in `data/ast.yaml`

Add `children` to `assign`, `block`, `proc`/`function`, `paren`, `return`, `type-annot`.

### 6. Acceptance: a second language

Define a language with renamed kinds (`set`/`params`/`body`) but identical roles; run it through the **unchanged** engines. If they handle it without edits, the hardcoding is gone. Gate for marking this ADR Accepted.

## Consequences

**Positive:** Closes the last engine hardcoding — "language as data" holds end-to-end. Zero kernel/native-op growth. Zero engine interpreter changes (under assembly-time resolution). Languages with different surface syntax reuse every engine without recompiles.

**Negative:** `AstShapeDef` gains a field. Assembly pass gains a role-resolution step (unresolved role = load-time error). `proj_child` payload becomes polymorphic (`u64` | `string`). Eight hardcoded sites need mechanical rewrites in the most-tested crates.

**Neutral:** Roles are per-shape, not global — `target` in `assign` is unrelated to `target` elsewhere. No global role registry; keeps the mechanism local.

## Implementation phases

0. **v2 audit** — check v2 engines for the same `kind ==` pattern. Scopes the rest.
1. **`AstShapeDef` roles + resolution** — schema + assembly pass. Gate: `cargo test -p handl-language`.
2. **Migrate `data/ast.yaml`** — declare roles. Gate: `cargo test -p handl-kit-data-canonical`.
3. **Rewrite primitives** — replace `kind ==` with role lookups; v2 only. Gate: existing tests pass unchanged.
4. **Cross-language test** — renamed kinds, same roles, unchanged engines. Gate: passes → mark Accepted.
5. **v1 deletion** — coordinated with ADR-0052 cutover. Gate: workspace builds without v1.

## Implementation evidence

**Phase 1–2** (prior session): `AstShapeDef.children` / `ChildRoleSpec` schema added; `Language::child_role_index(kind, role)` resolver added; roles declared in `data/ast.yaml` for `assign`, `block`, `function`, `return`, `type-annot`, and others.

**Phase 3** (this change): Replaced the remaining `kind == "paren" || kind == "array"` shape heuristics in the eval engines with `Language::has_variadic_role(kind)`, which inspects `AstShapeDef` role metadata. Sites converted:
- `plugin-engine-eval-cranelift` — two closure param-list sites (runtime + helper).
- `plugin-engine-eval-bytecode` — `vm.rs` closure handler; `compile.rs` `try_extract_closure_params_body` (threaded `&Language` through `compile`/`compile_into`/`compile_arr`).
- `handl-runtime/desugar_pass.rs` — `type-annot` detection now uses `child_role_index(kind, "annot")` first (threaded `&Language` through `try_rust_desugar`/`desugar_data`/`parse_data_payload`/`collect_constructors`/`desugar_match_to_recursor`).
- `plugin-ext-refinement/arrow_parser.rs` — binding detection now uses `child_role_index(kind, "target")` first.
- `data/ast.yaml` — added `variadic: true` to `paren`'s `body` role and a new `elements` variadic role to `array`, so collection-like shapes are detectable by role metadata alone.

All legacy `kind ==` checks are retained only as fallbacks for role-less shapes (backward compatibility).

**Phase 4** (this change): `crates/plugin-engine-eval-tree-walk/tests/adr0081_cross_language.rs` defines a language with renamed kinds (`set`/`block2`/`ret`/`params2`/`func2`) but identical roles and eval schemes, then runs it through the **unchanged** tree-walk and bytecode engines. Three tests cover block-with-binding and closure application. All pass — the engines handle renamed kinds without edits, satisfying the acceptance gate.

**Test results:** `cargo test -p handl-plugin-engine-eval-tree-walk -p handl-plugin-engine-eval-bytecode -p handl-plugin-engine-eval-cranelift -p handl-language -p handl-runtime` — all pass (0 failures).

## References

- [ADR-0044](./0044-language-as-data-and-runtime-split.md) — Language as Data.
- [ADR-0052](./0052-arrow-calculus-kernel.md) — Arrow Calculus Kernel (fixed 6-combinator kernel).
- [ADR-0053](./0053-generic-pratt-parser.md) — Generic Pratt Parser (desugar pass §7, prior `AstShapeDef` extension).
- [ADR-0054](./0054-generic-typecheck-engine.md) — Generic Typecheck Engine.
- [ADR-0058](./0058-generic-data-driven-engines.md) — Generic Data-Driven Engines (the scope this completes).
- [ADR-0059](./0059-generic-object-key-evaluation.md), [ADR-0060](./0060-generic-call-dispatch.md), [ADR-0061](./0061-generic-pattern-match.md) — prior primitive migrations.
- `.devin/rules/engine-genericity.md` — always-on rule codifying this invariant.
