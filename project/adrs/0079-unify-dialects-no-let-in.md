# ADR-0079: Unify Arrow Dialects — Graph Blocks, No `let`, No `in`

## Context

The handl platform has two dialects that share the same data-driven Pratt parser but diverge in surface syntax and post-parse handling:

1. **Main dialect** (`crates/handl-kit-data-canonical/data/*.yaml`): Used by ~55 runtime libs in `handl-packages/{dedent,stats,...}/*.hnl`, loaded via `HandlModule` (tree-walk evaluator). Uses `let x = y in body` for local bindings. Has full operators, literals, `op`/`handl`/`cap`/`law`/`trait`/`impl`/`infer`/`type` declarations, `while`/`for`/`do-while`, `import`/`export`.

2. **Arrow dialect** (`crates/plugin-ext-refinement/data/*.yaml`): Used by 23 files in `handl-packages/{collection,fn,state,...}/**/*.hnl`, loaded via `arrow_parser` → `ast_to_scheme` → `Scheme` IR → refinement inference. Uses `x = y in body` (no `let` keyword) and graph blocks `{ x = y; ... }`. Has `arr("op")`, `first(...)`, `comp(...)`, `choice(...)`, `app(...)`, `loop { state } ( body )`, `rec { state } ( body )`, `>>>` as `Seq`, `|||` as `Choice`. No runtime evaluator (eval.yaml is "DEFINITION ONLY").

The split creates maintenance burden, syntax inconsistency, and confusion. ADR-0074 already specifies the target surface (graph blocks, §7.1–7.2) but the engine hasn't fully implemented it.

## Decision

Merge into one unified dialect with all features from both, using graph blocks as the sole binding mechanism:

1. **Graph blocks replace `let..in` and `x = y in body`:**

   ```text
   -- before:  let x = f(a) in let y = g(x) in y
   -- after:   { x = f(a); y = g(x); y }
   ```

   `name = expr` lines inside `{ }`, separated by `;` or newline. Last expression is the block's value (ADR-0074 §7.1–7.2). No `let` keyword. No `in` keyword.

2. **Contextual `{}` parsing (ADR-0074 ID.11):** `{` after `~>`, `if cond`, `else`, `match val` parses as a _block_ (semicolon-separated, no key:value pairs). `{` elsewhere parses as a _record/object_. Implemented via a `block_children: [indices]` field on `AstShapeRef` that tells the parser which children should use block shape.

3. **`eval_block` binds assigns:** The tree-walk evaluator's `eval_block` pushes a scope, binds each `assign` child (`x = y`) as a local wire, and returns the last child's value. This makes graph blocks executable at runtime.

4. **Unified grammar (overlay approach):** The arrow DSL language is built on top of the canonical language: `arrow_lang.rs` calls `handl_kit_data_canonical::language_from_yaml()` as the base, then merges arrow-specific fragments (tokens, lexer rules, grammar rows, eval schemes, AST shapes) on top as an overlay. The `eval.arrow.flow` entry uses `replaces: ["eval.flow"]` to override canonical's `App` with `Seq` for `>>>`. The duplicate `grammar.keyword.return.prefix` row is removed from the arrow grammar (canonical provides it). Full deletion of the arrow YAMLs is deferred because the arrow dialect uses `.` in identifiers (`prim.compare`, `state.acc`) which is fundamentally incompatible with canonical's field-access `.` handling — merging the lexer rules would break one dialect or the other.

5. **`>>>` semantics:** Canonical uses `App` (host-call) for `>>>`; the arrow overlay replaces it with `Seq` (arrow composition) via `replaces`. The runtime never encounters `>>>` (no canonical `.hnl` file uses it). Full `Seq` semantics for the runtime is a future migration once the arrow-calculus kernel is productized.

6. **Remove `let` and `in`:** Delete `grammar.let.prefix` and `grammar.arrow.in.infix` after all files are converted.

## Consequences

- **Positive:** One dialect, one grammar, one set of YAML data files. No `let`/`in` keyword noise. Graph blocks are the single binding form, matching ADR-0074. Both runtime and refinement use the same parser/grammar.
- **Negative:** 6 `.hnl` files must be converted (`dedent`, `stats`, `arrows/fn/filter`, `arrows/fn/nested_map_rich`, `examples/handl/binary-search`, and verify `arrows/fn/zip_rich` + `arrows/fn/reduce_rich`). The `arrow_parser`/`ast_to_scheme` must be updated to handle unified node kinds. Existing `let`-using code breaks once `let` is removed (mitigated by additive migration: `let` stays valid until all files are converted).
- **Neutral:** `eval_let` becomes dead code (can be removed or left). The `>>>` host-call vs `Seq` semantics decision is deferred.

**Migration order (each step additive and reversible until the removal step):**

1. Engine: contextual `{}` + `eval_block` binds (additive, `let` still works) ✅
2. Convert `dedent.hnl` → test ✅
3. Convert `stats.hnl` → test ✅
4. Convert arrow files → refinement tests ✅
5. Remove `let` + `in` grammar rows → workspace test ✅
6. Arrow parser loads canonical as base + arrow overlay → refinement tests ✅
7. Delete arrow YAMLs, unify on canonical language → workspace test ✅

References: ADR-0074 (v0.1 surface syntax), ADR-0069 (files are arrows), ADR-0052 (arrow calculus kernel), ADR-0053 (data-driven Pratt parser).

## Status

Accepted

**Amendment (completed):** `let` is now fully removed end-to-end — the
statement form `let x :: v` (`token.let`, `lex.let`, `grammar.let.prefix`,
`ast.let`, `eval.let`, `type.let`) and the internal `stmt.let` kind
(`eval.stmt-let`; it had no producer — nothing desugared to it) are all
deleted. `token.keyword.let` + `lex.keyword.let` remain as a tombstone (the
`then`/`axiom` pattern: a keyword token with no grammar row) so `let` stays
a reserved word and `let x :: v` fails with "unexpected token" instead of
re-lexing as an identifier. Bindings are `x = y` / `x :: y` (`token.bind` →
`assign`) and `do { x = y; ... }` blocks. The dead `eval_let` dispatch arms
in the bytecode VM and cranelift engine are also removed.
