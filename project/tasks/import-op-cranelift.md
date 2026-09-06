# Task: Implement `import` and `eval_block` native ops in Cranelift (Phase 9.5)

## Objective

Bring `plugin-engine-eval-cranelift` up to parity with the tree-walk and
bytecode engines for Phase 9.5 import/export:

- Support `import "path"` in compiled/native programs.
- Support `do { stmt; expr }` blocks (`eval_block`) so that import statements
can appear in a block and the last expression can use imported bindings.

## Background

`crates/plugin-engine-eval-cranelift/src/lib.rs` defines:

- `CraneliftEngine` — the `EvalEngine` impl.
- `EvalCtx` — passed as a raw pointer to compiled native functions and back
  into `extern "C"` helper functions.
- `helper_v2_apply_native` / `apply_native_v2` — native op dispatcher.
- `ScopeStack` — the scope environment used by helpers.

The bytecode engine recently implemented the same two ops. See
`crates/plugin-engine-eval-bytecode/src/vm.rs` for reference, especially:

- `prim_v2_import`
- `prim_v2_eval_block`
- `role_child_arena`

## Subtasks

1. Add `import_resolver` to `CraneliftEngine` and `EvalCtx`.
   - Add `import_resolver: Option<Rc<dyn ImportResolver>>` to `CraneliftEngine`.
   - Override `EvalEngine::with_import_resolver` for `CraneliftEngine`.
   - Add `import_resolver: Option<*const dyn ImportResolver>` (or an `Rc`
     carried into `EvalCtx`) to `EvalCtx` and thread it through construction.
   - `EvalCtx::for_standalone` should default to no resolver.
   - Update `CraneliftEngine::eval` to create an `EvalCtx` with the resolver.

2. Implement `import` in `apply_native_v2`.
   - Add an `"import"` arm mirroring bytecode/tree-walk semantics:
     - Resolve module path from `args[0]` or `node.payload["path"]`.
     - Call `import_resolver.resolve_import(&path)`.
     - If `node.payload["alias"]` is present, bind the whole module object to
       that name; otherwise bind each top-level field of the returned object into
       the current scope.
     - Return `TaggedValue::Null`.
   - If no resolver is configured, return a `LangError` with a clear message.

3. Implement `eval_block` in `apply_native_v2`.
   - Add an `"eval_block"` arm.
   - Read `node.payload["children"]` array of arena indices.
   - Push a new scope, iterate children, return the last evaluated value.
   - Detect assign-like children by resolving the `target` and `value` child
     roles via `Language::child_role_index` (and payload fallback). For each
     binding child, evaluate the value and define it in the current scope.
   - Pop the scope before returning.

4. Tests.
   - Add unit tests in `plugin-engine-eval-cranelift/src/lib.rs` (or a new
     `tests/` file) that build a small AST and evaluate `stmt.import` and
     `block` nodes through `CraneliftEngine`.
   - Add a runtime integration test in `crates/handl-runtime/tests/` that uses
     the canonical language and `CraneliftEngine` to run
     `do { import "m"; x }` with a stub resolver returning `{ "x": 42 }`.

## Constraints

- Keep engine genericity: do not add `kind ==` checks for surface constructs.
  Use `Language::child_role_index` and payload fallbacks, mirroring the
  bytecode implementation.
- Compiled artifacts do not currently carry an import resolver; this is
  acceptable as a follow-up. The immediate goal is `EvalEngine::eval` parity.
- Follow existing cranelift patterns for raw pointers and `set_error`.
- Do not break existing `runtime_compile` or `cranelift` tests.

## Verification commands

```bash
cargo test -p handl-plugin-engine-eval-cranelift --lib
cargo test -p handl-runtime --test import_cranelift  # after adding integration test
cargo test -p handl-runtime --test runtime_compile
cargo test -p handl-kit-canonical
```

## References

- `crates/plugin-engine-eval-bytecode/src/vm.rs` — `prim_v2_import`,
  `prim_v2_eval_block`
- `crates/plugin-engine-eval-tree-walk/src/lib.rs` — `prim_import`,
  `prim_v2_eval_block`
- `crates/plugin-engine-eval-cranelift/src/lib.rs` — `EvalCtx`,
  `helper_v2_apply_native`, `apply_native_v2`
