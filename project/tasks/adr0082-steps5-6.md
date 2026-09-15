# ADR-0082 Steps 5–6: Inductive Families and Dependent Pattern Matching

Status: completed.

## Goal

Complete ADR-0082 build-order steps 5 and 6:

- **Step 5:** Value-indexed inductive families such as `Vec` and `Fin` desugar
  correctly from surface syntax to parameterized predicates, constructors, and
  recursors.
- **Step 6:** Surface `match` supports dependent motives and can be desugared
  into a recursor application over an indexed family.

All behavior must remain engine-generic: knowledge about `data`, `match`,
indices, and motives lives in `Language` data or in the desugar pass, not in
hardcoded engine `kind ==` checks.

## Subtasks

### 5.1 Indexed-family `data` declarations

Status: **completed**

- [x] Add an end-to-end test that parses and evaluates a value-indexed family such
      as `Vec` or `Fin` from source.
- Surface syntax example:

  ```text
  data Nat = Zero | Succ(Nat)
  data Vec(n: Nat, a: Type) = VNil : Vec(0, a)
                           | VCons(x: a, xs: Vec(n - 1, a)) : Vec(n, a)
  ```

- [x] Verify that the parser emits the `data` node shape used by
      `desugar_pass.rs` (`type` child is a `call` with `type-annot` parameters,
      `constructors` child is an `alt` tree with `type-annot` constructor result
      types).
- [x] Verify that `parse_data_payload`, `parse_type_expr`, `parse_type_param`,
      `collect_constructors`, `extract_result_indices_from_type`, and
      `build_predicate_body` handle index expressions such as `0`, `n`, and
      `n - 1` correctly.
- [x] Fix any issues found in `node_to_type_string` or `TypeExprParser` so that
      index arithmetic in constructor argument types and result types is preserved.
- [x] Manually call the generated recursor (`Vec.rec(null, base, step)(n, a, v)`)
      in the test to show that the generated predicate and recursor work without
      surface `match`.
- [x] Targeted verification:
  - `cargo test -p handl-runtime --test indexed_family_data`
  - `cargo test -p handl-runtime`
  - `cargo test -p handl-runtime --no-run`

### 5.2 Surface `match` over indexed families with motives

Status: **completed**

- [x] Extend `desugar_match_to_recursor` in `crates/handl-runtime/src/desugar_pass.rs`
      to handle indexed families (`params` non-empty).
- [x] Syntax supported:

  ```text
  match (v : Vec(n, a)) { VNil => ..., VCons(x, xs) => ... }
  match (v : Vec(n, a)) return (n, a, v) -> T with { ... }
  ```

- [x] Implementation notes applied:
  - The parser already supports the optional `return ... with` clause in
    `crates/plugin-engine-parse-pratt/src/lib.rs` (special-cased for `kind == "match"`).
  - If the scrutinee is a `paren` node, unwrap it.
  - If the scrutinee is a `type-annot` node, use the subject as the value and
    the `call(...)` type arguments as the index expressions for the recursor
    application.
  - If the family has no parameters, keep the existing non-indexed behavior.
  - If the family has parameters and the scrutinee is not type-annotated,
    leave the `match` unchanged (fall back to the runtime `match` primitive).
  - The motive, when present, is passed as the first argument to
    `Type.rec(motive, branches...)(indices..., value)`.
  - Engine genericity preserved: no `kind ==` checks added in eval engines;
    the desugar pass only builds ordinary `call`/`function`/`var` nodes.
- [x] Updated `crates/handl-kit-data-canonical/data/grammar.yaml` and
      `crates/handl-kit-data-canonical/data/ast.yaml` comments to accurately reflect
      the 2- or 3-child `match` shape and the optional `return`/`with` clause.
- [x] Added end-to-end test `crates/handl-runtime/tests/indexed_family_match.rs`
      covering plain and motive-bearing indexed `match` on `Vec`.
- [x] Verification commands run and passed:
  - `cargo test -p handl-runtime --test indexed_family_match`
  - `cargo test -p handl-runtime --test indexed_family_data`
  - `cargo test -p handl-runtime`
  - `cargo test -p handl-runtime --no-run`

### 5.3 Integration, `Fin` example, and ADR status

Status: **completed**

- [x] Added `crates/handl-runtime/tests/indexed_family_fin.rs` covering `Fin`
      predicate membership, surface `match` without a motive, and `match` with an
      explicit motive.
- [x] Updated `crates/handl-runtime/src/desugar_pass.rs` to guard `Nat`-indexed
      constructors against negative indices, so `Fin(0, FS(FZ))` correctly evaluates
      to `false` while `Fin(0, FZ)` and `Fin(1, FS(FZ))` evaluate to `true`.
- [x] Updated `project/adrs/0082-inductive-types.md` status from `Proposed` to
      `Accepted`.
- [x] Verification commands run:
  - `cargo test -p handl-runtime --test indexed_family_fin` — passed
  - `cargo test -p handl-runtime --test indexed_family_data` — passed
  - `cargo test -p handl-runtime --test indexed_family_match` — passed
  - `cargo test -p handl-runtime` — passed
  - `cargo test -p handl-plugin-engine-exec-tree-walk` — passed
  - `cargo test -p handl-plugin-engine-exec-bytecode` — passed
  - `cargo test -p handl-plugin-engine-exec-cranelift` — lib, conformance, and
    object-compilation tests passed; `tests/standalone_bootstrap.rs` timed out
    after 180 s (pre-existing hang, unrelated to ADR-0082).

## Related files

- `crates/handl-runtime/src/desugar_pass.rs`
  - `parse_data_payload` (line ~792)
  - `parse_type_expr` / `parse_type_param` (line ~861)
  - `collect_constructors` / `extract_result_indices_from_type` (line ~914)
  - `node_to_type_string` (line ~1107)
  - `desugar_data` (line ~1553)
  - `build_predicate_body` / `build_cons_check` (line ~1633)
  - `build_recursor` / `build_recursor_dispatch` (line ~1725)
  - `desugar_match_to_recursor` (line ~1940)
- `crates/plugin-engine-parse-pratt/src/lib.rs` (`match` and `data` special cases)
- `crates/handl-kit-data-canonical/data/grammar.yaml`
- `crates/handl-kit-data-canonical/data/ast.yaml`
- `crates/handl-kit-data-canonical/data/eval.yaml`
- `crates/handl-runtime/tests/` (add new test files or extend existing ones)
- `project/adrs/0082-inductive-types.md`

## Notes

- The parser already special-cases `match` for optional `return ... with` and
  `data` for optional parenthesised parameters. These contextual branches are
  intentional (ADR-0053 §contextual disambiguation), but the canonical YAML
  should still document the resulting AST shape accurately.
- Do not duplicate the already-completed ADR-0082 steps 1–4 work. Before
  changing `desugar_pass.rs`, confirm the existing code handles the
  non-indexed `Nat`/`Bool` cases correctly.
- Avoid full-workspace builds while stale cargo locks or unrelated uncommitted
  changes may be present. Prefer targeted crate tests and `cargo test --no-run`.
