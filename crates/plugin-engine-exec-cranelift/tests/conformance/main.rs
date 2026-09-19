//! Cross-interpreter conformance suite (Phase A — evaluator
//! unification).
//!
//! Runs identical [`plugin_engine::TermData`] inputs through all four
//! evaluators — tree-walk, bytecode, Cranelift, and the pure
//! `plugin_engine::term_interpreter` — and diffs the results.
//!
//! * `cargo test -p handl-plugin-engine-exec-cranelift --test conformance`
//!   asserts the pinned matrix (any `FAIL` cell = undocumented drift).
//! * `cargo test -p handl-plugin-engine-exec-cranelift --test conformance
//!   -- --nocapture` prints the full report table with actual outcomes
//!   for every divergent/unsupported cell.
//!
//! # Findings (Phase-A baseline for Phase B deduplication)
//!
//! ## Suspected bugs
//!
//! * **Bytecode incoming plumbing** (`plugin-engine-exec-bytecode/src/vm.rs`,
//!   `op_uses_incoming`): the 0-arg `NativeOp` arm reads `incoming` only
//!   for a hardcoded op list.
//!   - Ops that consume `incoming` but are absent from the list
//!     (`first`, `sum`, `app`, `compose`) silently receive
//!     `current_incoming` — `None` under `Seq` — instead of the
//!     predecessor's value: `seq_first_incoming`, `seq_app_incoming`,
//!     `seq_sum_incoming`, `compose_pipe`.
//!   - Ops *on* the list used in a later arg position pop whatever the
//!     previous arg pushed off the operand stack — masked for `id`
//!     (`seq_id_later_arg`), corrupting for `inc` (`seq_inc_later_arg`,
//!     `sub(10, inc)` on input 7 → `-4` instead of `2`) and fatal for
//!     the `Seq{axis>0}` → `cell_comp_axis:<n>(axis, id, id)` lowering
//!     (`seq_axis1_dim2`, `seq_axis2_dim3` → stack underflow).
//! * **Bytecode n-ary convention**: typed instructions (`Sub`,
//!   `Arith(And/Or)`) consume the *last two* stack operands, while
//!   `native_ops` reads `args[0..2]` and `term_interpreter` folds all
//!   args — `sub(10,3,2)` yields `7`/`1`/`5` across the three paths:
//!   `sub_nary`, `and_extra_arg`, `or_extra_arg`.
//! * **Bytecode `Loop` underflow**: the `Push(Null); Pair` prologue
//!   requires a stack-carried incoming — a top-level `Loop` with no
//!   input crashes (`loop_no_incoming`); the other paths treat missing
//!   incoming as `null`.
//! * **`native_ops::nullish`**: a non-null, non-`Some` left operand
//!   returns `right` (`3 ?? 7` → `7`, `nullish_value`). All three
//!   engines agree because they share the table — drift-free but
//!   suspect semantics (expected `left`).
//!
//! ## Representation splits (pinned; Phase-B unification targets)
//!
//! * `product`/`Term::First` pairs: engines emit `{"__pair__":[a,b]}`;
//!   `term_interpreter`'s `mk_pair` is a bare JSON array (`product`,
//!   `product_extra_arg`, `first_combinator`).
//! * `term_interpreter` arithmetic accumulates in `f64`: string `add`
//!   fails (`add_str`), `7/2` → `3.5` vs engines' truncating `3`
//!   (`div_int_trunc`).
//! * Callable-value shapes: engines apply `{"__arrow__":…}` closures
//!   (`app_arrow_callee`, `app_bound_record`); `term_interpreter`
//!   applies serialized `Term`/`__term__` callees (`app_term_callee`).
//!   Neither recognizes the other's shape.
//! * `tag` stamping: engines add `__registry__: handl.canonical`;
//!   `term_interpreter` only inserts `__tag__` (`tag_construct`).
//! * `pair_tag` exists only in `term_interpreter`'s op table; `record`
//!   exists only in tree-walk + `term_interpreter` (`pair_tag`,
//!   `record_op`).
//!
//! ## Two `Handle` models (documented intentional divergence)
//!
//! Compiled backends *compile-time eliminate* `Handle`, substituting the
//! `impl_` term for handled ops; tree-walk/`term_interpreter` evaluate
//! `impl_` to a runtime handler record with `grades`/`impls` and dispatch
//! dynamically. Both directions pinned: `handle_record`,
//! `handle_term_subst`.
//!
//! ## Capability gaps (`UNSUPPORTED`)
//!
//! * `term_interpreter` has no arms for `mod`, `neg`, `pow`,
//!   `number_add`, `neq`, `compare`, `not`, `inc`, `dec`, `is_pos`,
//!   `half_floor`, `inc_fst`, `countdown_step`, `infinite_step`,
//!   `first`/`second`/`left`/`right`/`sum`/`app` (as ops), `const`,
//!   `extend_env`, `proj_env`, `proj_child_or`, `paren`, `typeof`,
//!   `nullish`, `opt_chain`, `index`, `while_step`, `loop_step`,
//!   `fold_step` — and rejects `Term::Child`/`Term::Iter`.
//! * Bytecode and Cranelift compile `Term::Child`/`Term::Iter` to
//!   `NotImplemented` and have no `record` op.
//! * `EvalNode`-gated forms (`paren`, `index` node-form,
//!   `proj_child`) are engine-only by construction — the term
//!   interpreter has no AST arena.

mod cases;
mod harness;

/// Assert the pinned matrix: every cell is PASS, DIVERGENT, or
/// UNSUPPORTED — `FAIL` means an expectation no longer matches reality
/// (undocumented drift or a silently fixed divergence; either way the
/// baseline needs an explicit update).
#[test]
fn conformance_matrix() {
    let rows = harness::run_suite(&cases::cases());
    let report = harness::format_report(&rows);
    let fails = harness::failures(&rows);
    assert!(
        fails.is_empty(),
        "cross-interpreter conformance failures:\n{}\n\n{report}",
        fails.join("\n")
    );
    // Sanity: the suite actually exercised every interpreter.
    assert!(
        rows.iter()
            .all(|r| r.cells.len() == harness::INTERPRETERS.len()),
        "every row must have one cell per interpreter"
    );
}

/// Print-only entry point (`--nocapture`): the full matrix plus actual
/// outcomes on every non-PASS cell.
#[test]
fn report() {
    let rows = harness::run_suite(&cases::cases());
    println!("\n{}", harness::format_report(&rows));
}
