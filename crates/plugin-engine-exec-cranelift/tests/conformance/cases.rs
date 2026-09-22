//! Golden corpus for the cross-interpreter conformance harness.
//!
//! Each [`Case`] is a [`TermData`] body run through all four evaluators
//! (tree-walk, bytecode, cranelift, term-interp — see
//! [`crate::harness::INTERPRETERS`]) under an optional `incoming` value.
//! Expectations are *pinned*: a divergence that is already documented is
//! `DivVal`/`DivErr`; an unanticipated one fails the suite.
//!
//! Divergence notes cite the implementation site so a Phase-B reviewer can
//! jump straight to the drift.

use plugin_engine::{ArrSource, TermData};
use serde_json::{Value, json};

use crate::harness::{Case, Expect, ProbeNode};

// ---------------------------------------------------------------------------
// Term builders
// ---------------------------------------------------------------------------

/// `Arr` — pure literal value.
fn lit(v: impl Into<Value>) -> TermData {
    TermData::arr(v)
}

/// Native op, no literal.
fn op(name: &str, args: Vec<TermData>) -> TermData {
    TermData::op(name, args, &ArrSource::Native, None)
}

/// Native op with a `lit` literal (`proj_env "x"`, `proj_child 0`, …).
fn op_lit(name: &str, lit: Value) -> TermData {
    TermData::op(name, vec![], &ArrSource::Native, Some(lit))
}

/// Native op at declared dimension `d` (`op_dim`).
fn op_at(name: &str, d: u64, args: Vec<TermData>) -> TermData {
    TermData::op_dim(name, TermData::dim_const(d), args, &ArrSource::Native, None)
}

/// Native op at a *named* dimension/grade (`op_dim` with `Const "S"`).
fn op_at_label(name: &str, label: &str) -> TermData {
    TermData::op_dim(
        name,
        TermData::dim_const(label),
        vec![],
        &ArrSource::Native,
        None,
    )
}

/// `id` at declared dimension `d` — a dimension-lifted identity.
fn id_at(d: u64) -> TermData {
    op_at("id", d, vec![])
}

/// Wire `{"__pair__": [a, b]}` value.
fn pairv(a: impl Into<Value>, b: impl Into<Value>) -> Value {
    json!({ "__pair__": [a.into(), b.into()] })
}

/// Wire `{"__either__": "left", "value": v}`.
fn eleft(v: impl Into<Value>) -> Value {
    json!({ "__either__": "left", "value": v.into() })
}

/// Wire `{"__either__": "right", "value": v}`.
fn eright(v: impl Into<Value>) -> Value {
    json!({ "__either__": "right", "value": v.into() })
}

/// Wire `Cell2{src, tgt, value}` record.
fn cell2v(s: impl Into<Value>, t: impl Into<Value>, v: impl Into<Value>) -> Value {
    json!({ "__tag__": "Cell2", "__src__": s.into(), "__tgt__": t.into(), "value": v.into() })
}

/// Wire `Cell{n, src, tgt, value}` record (`n >= 3`).
fn cellnv(n: u64, s: Value, t: Value, v: Value) -> Value {
    json!({ "__tag__": "Cell", "__dim__": n, "__src__": s, "__tgt__": t, "value": v })
}

/// A `{"__tag__": "Number", "value": n}` tagged operand.
fn num_tag(n: i64) -> Value {
    json!({ "__tag__": "Number", "value": n })
}

/// A closure-value fixture `{"__arrow__": true, "params": [...], "body":
/// <probe-node idx>}` — a *marker* shape that `build_probe` rewrites into
/// the self-contained ADR-0106 wire form (`body_def` = one-node `__ast__`
/// arena holding `nodes[body]`). Fixture bodies are always scheme-carrying
/// leaf nodes; their schemes resolve against the ambient probe `Language`
/// via `node.kind`.
fn arrow(params: &[&str], body: usize) -> Value {
    let ps: Vec<Value> = params.iter().map(|p| json!(p)).collect();
    json!({ "__arrow__": true, "params": ps, "body": body })
}

/// Same, but an array-pattern arrow (`[a, b] ~> body` destructures the
/// single array argument by position).
fn arrow_pat(params: &[&str], body: usize) -> Value {
    let ps: Vec<Value> = params.iter().map(|p| json!(p)).collect();
    json!({ "__arrow__": true, "params": ps, "body": body, "array_pattern": true })
}

/// Serialized `Term` value — the callee shape `term_interpreter`'s
/// `Term::App` applies (`from_value::<Term>`).
fn term_value(td: TermData) -> Value {
    serde_json::to_value(td.to_term().expect("TermData -> Term")).expect("Term -> Value")
}

/// Probe node: a scheme-less literal leaf (engines return the payload;
/// `proj_child`/`paren`/`index` fixtures).
fn leaf(payload: Value) -> ProbeNode {
    ProbeNode {
        kind: "lit",
        payload,
        scheme: None,
        roles: Vec::new(),
        variadic_roles: Vec::new(),
        pattern_role: None,
        literal_children: Vec::new(),
    }
}

/// Probe node carrying its own eval scheme (closure bodies).
fn schemed(kind: &'static str, scheme: TermData) -> ProbeNode {
    ProbeNode {
        kind,
        payload: Value::Null,
        scheme: Some(scheme),
        roles: Vec::new(),
        variadic_roles: Vec::new(),
        pattern_role: None,
        literal_children: Vec::new(),
    }
}

/// Probe `arm`/`clause` node (`match`/`handle`): `pattern`/`body` child
/// roles (ADR-0081) with absolute arena indices.
fn arm_node(pattern_idx: usize, body_idx: usize) -> ProbeNode {
    ProbeNode {
        kind: "pb.arm",
        payload: json!({"children": [pattern_idx, body_idx]}),
        roles: vec![("pattern", false), ("body", false)],
        ..leaf(Value::Null)
    }
}

/// Probe arms-block node (`match`/`handle`): children are arm arena
/// indices; no roles — the shell reads them positionally.
fn block_node(arm_indices: &[usize]) -> ProbeNode {
    ProbeNode {
        kind: "pb.block",
        payload: json!({"children": arm_indices}),
        ..leaf(Value::Null)
    }
}

/// Probe `var` node — the role-less leaf-binder pattern fallback
/// (`pattern_role: leaf` on canonical `var`; used unnamed here so the
/// `kind == "var"` fallback path is exercised too).
fn var_node(name: &'static str) -> ProbeNode {
    ProbeNode {
        kind: "var",
        payload: json!(name),
        ..leaf(Value::Null)
    }
}

/// Probe node for an object `entry` child (`k: v`): `key`/`value` roles,
/// `pattern_role: entry`, and `literal_children: {key: [lit]}` so `lit`
/// key children are read as literal key text (ADR-0081 metadata, not a
/// kind check). `key_idx`/`val_idx` are absolute arena indices.
fn entry_node(key_idx: usize, val_idx: usize) -> ProbeNode {
    ProbeNode {
        kind: "pb.entry",
        payload: json!({"children": [key_idx, val_idx]}),
        roles: vec![("key", false), ("value", false)],
        pattern_role: Some("entry"),
        literal_children: vec![("key", vec!["lit"])],
        ..leaf(Value::Null)
    }
}

// ---------------------------------------------------------------------------
// Expectation builders
// ---------------------------------------------------------------------------

/// All four interpreters produce `v`.
fn all_ok(v: Value) -> [Expect; 4] {
    [
        Expect::Val(v.clone()),
        Expect::Val(v.clone()),
        Expect::Val(v.clone()),
        Expect::Val(v),
    ]
}

/// All four reject the case; each `Err` contains `sub`.
fn all_err(sub: &'static str) -> [Expect; 4] {
    [
        Expect::Err(sub),
        Expect::Err(sub),
        Expect::Err(sub),
        Expect::Err(sub),
    ]
}

/// The three `EvalEngine`s produce `v`; the term interpreter lacks the op.
fn engines_ok(v: Value) -> [Expect; 4] {
    [
        Expect::Val(v.clone()),
        Expect::Val(v.clone()),
        Expect::Val(v),
        Expect::Unsupported,
    ]
}

/// Only tree-walk produces `v`; the other three cannot run the case.
fn tw_only(v: Value) -> [Expect; 4] {
    [
        Expect::Val(v),
        Expect::Unsupported,
        Expect::Unsupported,
        Expect::Unsupported,
    ]
}

/// Engines produce `v`; the term interpreter pins a divergent `Ok(w)`.
fn engines_ok_ti_div(v: Value, w: Value) -> [Expect; 4] {
    [
        Expect::Val(v.clone()),
        Expect::Val(v.clone()),
        Expect::Val(v),
        Expect::DivVal(w),
    ]
}

/// Engines produce `v`; the term interpreter pins a divergent `Err`.
fn engines_ok_ti_err(v: Value, sub: &'static str) -> [Expect; 4] {
    [
        Expect::Val(v.clone()),
        Expect::Val(v.clone()),
        Expect::Val(v),
        Expect::DivErr(sub),
    ]
}

fn case(
    name: &'static str,
    category: &'static str,
    body: TermData,
    input: Option<Value>,
    expect: [Expect; 4],
    note: &'static str,
) -> Case {
    Case {
        name,
        category,
        body,
        input,
        nodes: Vec::new(),
        roles: Vec::new(),
        optional_roles: Vec::new(),
        root_payload: None,
        expect,
        note,
    }
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// `x ~> x + 10` body scheme for `arrow(&["x"], 0)` fixtures.
fn inc10_scheme() -> TermData {
    op("add", vec![op_lit("proj_env", json!("x")), lit(10)])
}

/// `x ~> x + 100`.
fn add100_scheme() -> TermData {
    op("add", vec![op_lit("proj_env", json!("x")), lit(100)])
}

/// The shared corpus. Column order: `[tree-walk, bytecode, cranelift,
/// term-interp]`.
pub fn cases() -> Vec<Case> {
    let mut out: Vec<Case> = Vec::new();

    // ================= arithmetic / comparison / logic =================
    let cat = "arith";
    out.push(case(
        "add",
        cat,
        op("add", vec![lit(2), lit(3)]),
        None,
        all_ok(json!(5)),
        "",
    ));
    out.push(case(
        "add_float",
        cat,
        op("add", vec![lit(2.5), lit(1)]),
        None,
        all_ok(json!(3.5)),
        "",
    ));
    out.push(case(
        "add_str",
        cat,
        op("add", vec![lit("a"), lit("b")]),
        None,
        [
            Expect::Val(json!("ab")),
            Expect::Val(json!("ab")),
            Expect::Val(json!("ab")),
            Expect::DivErr("non-numeric"),
        ],
        "engines: bin_add concatenates strings; term-interp arithmetic is \
         f64-only (`as_f64` fails on strings) — known representation split.",
    ));
    out.push(case(
        "sub",
        cat,
        op("sub", vec![lit(10), lit(3)]),
        None,
        all_ok(json!(7)),
        "",
    ));
    out.push(case(
        "sub_nary",
        cat,
        op("sub", vec![lit(10), lit(3), lit(2)]),
        None,
        all_ok(json!(5)),
        "documented n-ary convention — left-fold `((10-3)-2)` = 5. \
         `native_ops::bin_num` folds all args; bytecode emits the typed \
         `Sub` instr only for exact arity 2 (n-ary falls through to the \
         folding `NativeOp` path); term-interp folds natively.",
    ));
    out.push(case(
        "mul",
        cat,
        op("mul", vec![lit(4), lit(5)]),
        None,
        all_ok(json!(20)),
        "",
    ));
    out.push(case(
        "div_even",
        cat,
        op("div", vec![lit(8), lit(2)]),
        None,
        all_ok(json!(4)),
        "",
    ));
    out.push(case(
        "div_int_trunc",
        cat,
        op("div", vec![lit(7), lit(2)]),
        None,
        engines_ok_ti_div(json!(3), json!(3.5)),
        "engines: int/int is truncating integer division \
         (native_ops Num::Div); term-interp accumulates in f64 → 3.5. \
         Known representation split — flagged for Phase B.",
    ));
    out.push(case(
        "div_zero",
        cat,
        op("div", vec![lit(1), lit(0)]),
        None,
        [
            Expect::Err("division by zero"),
            Expect::Err("division by zero"),
            Expect::Err("division by zero"),
            Expect::Err("invalid numeric result"),
        ],
        "all reject; term-interp produces inf via f64 then fails \
         `Number::from_f64` — different message, same verdict.",
    ));
    out.push(case(
        "mod",
        cat,
        op("mod", vec![lit(10), lit(3)]),
        None,
        engines_ok(json!(1)),
        "term-interp has no `mod` arm.",
    ));
    out.push(case(
        "neg",
        cat,
        op("neg", vec![lit(4)]),
        None,
        engines_ok(json!(-4)),
        "term-interp has no `neg` arm.",
    ));
    out.push(case(
        "pow",
        cat,
        op("pow", vec![lit(2), lit(10)]),
        None,
        engines_ok(json!(1024.0)),
        "engines return Float (pow always lowers through f64); \
         term-interp has no `pow` arm.",
    ));
    out.push(case(
        "number_add",
        cat,
        op("number_add", vec![lit(num_tag(2)), lit(num_tag(3))]),
        None,
        engines_ok(json!({"__tag__": "Number", "value": 5})),
        "typed `Number`-record add — engines only; term-interp lacks the op.",
    ));
    out.push(case(
        "eq",
        cat,
        op("eq", vec![lit(2), lit(2)]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "eq_arr",
        cat,
        op("eq", vec![lit(json!([1, 2])), lit(json!([1, 2]))]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "neq",
        cat,
        op("neq", vec![lit(2), lit(3)]),
        None,
        engines_ok(json!(true)),
        "term-interp has no `neq` arm.",
    ));
    out.push(case(
        "lt",
        cat,
        op("lt", vec![lit(1), lit(2)]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "gt",
        cat,
        op("gt", vec![lit(5), lit(4)]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "lte",
        cat,
        op("lte", vec![lit(2), lit(2)]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "gte",
        cat,
        op("gte", vec![lit(3), lit(3)]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "compare",
        cat,
        op("compare", vec![lit(3), lit(4)]),
        None,
        engines_ok(json!(-1)),
        "term-interp has no `compare` arm.",
    ));
    out.push(case(
        "not",
        cat,
        op("not", vec![lit(false)]),
        None,
        engines_ok(json!(true)),
        "term-interp has no `not` arm.",
    ));
    out.push(case(
        "and",
        cat,
        op("and", vec![lit(true), lit(true)]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "and_extra_arg",
        cat,
        op("and", vec![lit(true), lit(true), lit(false)]),
        None,
        all_ok(json!(false)),
        "n-ary `and` = `all(truthy)` on every engine (left-fold \
         convention). Bytecode emits `Arith(And)` only at exact arity 2; \
         n-ary falls through to the folding `NativeOp` path.",
    ));
    out.push(case(
        "or",
        cat,
        op("or", vec![lit(false), lit(true)]),
        None,
        all_ok(json!(true)),
        "",
    ));
    out.push(case(
        "or_extra_arg",
        cat,
        op("or", vec![lit(false), lit(false), lit(true)]),
        None,
        all_ok(json!(true)),
        "n-ary `or` = `any(truthy)` on every engine (left-fold \
         convention). Bytecode emits `Arith(Or)` only at exact arity 2; \
         n-ary falls through to the folding `NativeOp` path.",
    ));

    // ================= incoming plumbing / Seq axis-0 =================
    let cat = "incoming";
    out.push(case(
        "id",
        cat,
        op("id", vec![]),
        Some(json!(42)),
        all_ok(json!(42)),
        "canonical incoming read.",
    ));
    out.push(case(
        "inc",
        cat,
        op("inc", vec![]),
        Some(json!(3)),
        engines_ok(json!(4)),
        "",
    ));
    out.push(case(
        "dec",
        cat,
        op("dec", vec![]),
        Some(json!(3)),
        engines_ok(json!(2)),
        "",
    ));
    out.push(case(
        "is_pos",
        cat,
        op("is_pos", vec![]),
        Some(json!(3)),
        engines_ok(json!(true)),
        "",
    ));
    out.push(case(
        "is_pos_neg",
        cat,
        op("is_pos", vec![]),
        Some(json!(-2)),
        engines_ok(json!(false)),
        "",
    ));
    out.push(case(
        "half_floor",
        cat,
        op("half_floor", vec![]),
        Some(json!(7)),
        engines_ok(json!(3)),
        "",
    ));
    out.push(case(
        "tag",
        cat,
        op("tag", vec![]),
        Some(json!(5)),
        all_ok(eleft(5)),
        "truthy → Left(v) — same `__either__` wire shape on all four.",
    ));
    out.push(case(
        "tag_falsy",
        cat,
        op("tag", vec![]),
        Some(json!(0)),
        all_ok(eright(0)),
        "falsy → Right(v).",
    ));
    out.push(case(
        "inc_fst",
        cat,
        op("inc_fst", vec![]),
        Some(pairv(5, 9)),
        engines_ok(pairv(6, 9)),
        "pair-in/pair-out step helper.",
    ));
    out.push(case(
        "countdown_step",
        cat,
        op("countdown_step", vec![]),
        Some(pairv(pairv(42, 3), 2)),
        engines_ok(eright(1)),
        "loop-pair `((result, initial), state)` — state 2 > 0 → Right(1).",
    ));
    out.push(case(
        "infinite_step",
        cat,
        op("infinite_step", vec![]),
        Some(pairv(7, 8)),
        engines_ok(eright(8)),
        "always Right(state).",
    ));
    out.push(case(
        "seq_id_first_arg",
        cat,
        op("add", vec![op("id", vec![]), lit(1)]),
        Some(json!(7)),
        all_ok(json!(8)),
        "control: `id` in arg position 0 reads the Seq predecessor's value \
         on all four (bytecode pops it off the operand stack).",
    ));
    out.push(case(
        "seq_id_later_arg",
        cat,
        op("add", vec![lit(1), op("id", vec![])]),
        Some(json!(7)),
        all_ok(json!(8)),
        "`id` in arg position 1 reads the region's current incoming \
         (`7`) via `PushIncoming` + `NativeOp` — `PushIncoming` pushes a \
         copy so the earlier `1` arg stays on the stack for `add`.",
    ));
    out.push(case(
        "seq_inc_later_arg",
        cat,
        op("sub", vec![lit(10), op("inc", vec![])]),
        Some(json!(7)),
        [
            Expect::Val(json!(2)),
            Expect::Val(json!(2)),
            Expect::Val(json!(2)),
            Expect::Unsupported,
        ],
        "`inc` in arg position 1 reads the region's current incoming — \
         the Seq predecessor `7` — via `PushIncoming` + `NativeOp` → \
         10 - 8 = 2 on all engines. term-interp has no `inc` op.",
    ));
    out.push(case(
        "seq_first_incoming",
        cat,
        op("first", vec![lit(arrow(&["x"], 0))]),
        Some(pairv(1, 99)),
        [
            Expect::Val(pairv(11, 99)),
            Expect::Val(pairv(11, 99)),
            Expect::Val(pairv(11, 99)),
            Expect::Unsupported,
        ],
        "`first` is not in `op_uses_incoming` — it receives incoming via \
         the `current_incoming` register, which `SetIncoming` updates to \
         the Seq predecessor's pair. term-interp lacks the `first` op.",
    ));
    out.push(case(
        "seq_app_incoming",
        cat,
        op("app", vec![lit(arrow(&["x"], 0))]),
        Some(json!(41)),
        [
            Expect::Val(json!(51)),
            Expect::Val(json!(51)),
            Expect::Val(json!(51)),
            Expect::Unsupported,
        ],
        "`app` gets the Seq predecessor `41` via `current_incoming` on \
         all engines → the `x+10` arrow yields 51.",
    ));
    out.push(case(
        "seq_sum_incoming",
        cat,
        op("sum", vec![lit(arrow(&["x"], 0)), lit(arrow(&["x"], 1))]),
        Some(eleft(5)),
        [
            Expect::Val(eleft(15)),
            Expect::Val(eleft(15)),
            Expect::Val(eleft(15)),
            Expect::Unsupported,
        ],
        "same incoming plumbing as `seq_first_incoming` — `sum` sees the \
         Seq predecessor's `Either` on all engines.",
    ));

    // ================= products / sums =================
    let cat = "product";
    out.push(case(
        "product",
        cat,
        op("product", vec![lit(1), lit(2)]),
        None,
        engines_ok_ti_div(pairv(1, 2), json!([1, 2])),
        "pair wire shape: engines emit `{\"__pair__\":[a,b]}` \
         (TaggedValue::Pair); term-interp's `mk_pair` is a plain JSON \
         array — known representation split.",
    ));
    out.push(case(
        "product_extra_arg",
        cat,
        op("product", vec![lit(1), lit(2), lit(3)]),
        None,
        engines_ok_ti_div(pairv(1, 2), json!([1, 2, 3])),
        "engines keep args[0..2] only; term-interp's `product`/`array` arm \
         returns *all* args — SUSPECTED DRIFT on >2 args.",
    ));
    out.push(case(
        "first_combinator",
        cat,
        TermData::first(op("add", vec![op("id", vec![]), lit(1)])),
        Some(pairv(5, 99)),
        engines_ok_ti_div(pairv(6, 99), json!([6, 99])),
        "`Term::First` works on all four; term-interp re-pairs as a bare \
         array — same pair-representation split as `product`.",
    ));
    out.push(case(
        "first_op",
        cat,
        op("first", vec![lit(arrow(&["x"], 0)), lit(pairv(1, 99))]),
        None,
        engines_ok(pairv(11, 99)),
        "explicit-input `first(f, pair)` — avoids the incoming-plumbing \
         gap so all three engines agree.",
    ));
    out.push(case(
        "second_op",
        cat,
        op("second", vec![lit(arrow(&["x"], 0)), lit(pairv(1, 99))]),
        None,
        engines_ok(pairv(1, 109)),
        "",
    ));
    out.push(case(
        "left_op",
        cat,
        op("left", vec![lit(arrow(&["x"], 0)), lit(eleft(5))]),
        None,
        engines_ok(eleft(15)),
        "",
    ));
    out.push(case(
        "right_op",
        cat,
        op("right", vec![lit(arrow(&["x"], 1)), lit(eright(5))]),
        None,
        engines_ok(eright(105)),
        "",
    ));
    out.push(case(
        "sum_op",
        cat,
        op(
            "sum",
            vec![lit(arrow(&["x"], 0)), lit(arrow(&["x"], 1)), lit(eleft(5))],
        ),
        None,
        engines_ok(eleft(15)),
        "explicit-input `sum(f, g, either)` — args[2] supplies the either \
         so all three engines agree.",
    ));
    out.push(case(
        "choice_left",
        cat,
        TermData::choice(
            op("add", vec![op("id", vec![]), lit(10)]),
            op("add", vec![op("id", vec![]), lit(100)]),
        ),
        Some(eleft(5)),
        all_ok(json!(15)),
        "`Term::Choice` routes on `__either__` identically on all four.",
    ));
    out.push(case(
        "choice_right",
        cat,
        TermData::choice(
            op("add", vec![op("id", vec![]), lit(10)]),
            op("add", vec![op("id", vec![]), lit(100)]),
        ),
        Some(eright(5)),
        all_ok(json!(105)),
        "",
    ));
    out.push(case(
        "app_op",
        cat,
        op("app", vec![lit(arrow(&["x"], 0)), lit(41)]),
        None,
        engines_ok(json!(51)),
        "explicit-arg `app(f, x)` — term-interp has `Term::App` but not \
         the `app` op.",
    ));
    out.push(case(
        "app_arrow_callee",
        cat,
        TermData::app(lit(arrow(&["x"], 0)), vec![lit(41)]),
        None,
        engines_ok_ti_err(json!(51), "arrow"),
        "`Term::App` on an `__arrow__` closure value: engines apply it \
         via `apply_app`; term-interp only applies serialized `Term` / \
         `__term__` callees — representation split (its `__arrow__` \
         support is absent).",
    ));
    out.push(case(
        "app_term_callee",
        cat,
        TermData::app(
            lit(term_value(op("add", vec![op("id", vec![]), lit(1)]))),
            vec![lit(41)],
        ),
        None,
        [
            Expect::DivErr("arrow"),
            Expect::DivErr("arrow"),
            Expect::DivErr("arrow"),
            Expect::Val(json!(42)),
        ],
        "inverse of `app_arrow_callee`: term-interp applies a serialized \
         `Term` callee; the engines' `apply_app` does not recognize the \
         `{\"__tag__\":\"Op\",…}` shape as callable — representation \
         split, both directions pinned.",
    ));
    out.push(case(
        "app_bound_record",
        cat,
        TermData::app(
            lit(json!({
                "__term__": arrow_pat(&["a", "b"], 2),
                "__bound__": [100],
            })),
            vec![lit(41)],
        ),
        None,
        [
            Expect::Val(json!(141)),
            Expect::Val(json!(141)),
            Expect::Val(json!(141)),
            Expect::DivErr("arrow"),
        ],
        "engines: `{__term__, __bound__}` applies the arrow to \
         bound++args delivered as ONE array (`array_pattern` \
         destructures [100,41] → a,b → 141). term-interp tries \
         `from_value::<Term>` on the `__arrow__` record and fails — \
         representation split.",
    ));
    out.push(case(
        "pair_tag",
        cat,
        op("pair_tag", vec![]),
        Some(pairv(true, 7)),
        all_ok(eleft(7)),
        "`pair_tag` (used by the `if` lowering) is implemented on all \
         four — routes a pair's `snd` into an `Either` by its `fst` \
         discriminant.",
    ));

    // ================= environment / structural =================
    let cat = "env";
    out.push(case(
        "const_op",
        cat,
        op_lit("const", json!(42)),
        None,
        engines_ok(json!(42)),
        "engines: `const` reads `lit`; term-interp has no `const` op arm \
         (its `Term::Const`/`Term::Arr` carry the value directly).",
    ));
    out.push(case(
        "extend_env_proj_env",
        cat,
        TermData::seq(
            op_lit("extend_env", json!("x")),
            op_lit("proj_env", json!("x")),
        ),
        Some(json!(99)),
        engines_ok(json!(99)),
        "seq pushes the scope `extend_env` opened; `proj_env` reads it \
         back. term-interp has no environment.",
    ));
    out.push(case(
        "proj_env_unbound",
        cat,
        op_lit("proj_env", json!("zzz")),
        None,
        engines_ok(json!("zzz")),
        "unbound names resolve to the name string itself (documented \
         `proj_env` fallback feeding host-function dispatch).",
    ));
    out.push(case(
        "proj_child",
        cat,
        op_lit("proj_child", json!(1)),
        Some(json!([10, 20, 30])),
        [
            Expect::Val(json!(20)),
            Expect::Val(json!(20)),
            Expect::Val(json!(20)),
            Expect::Val(json!(20)),
        ],
        "engines evaluate arena child 1; term-interp projects index 1 of \
         the incoming tuple — different notions of 'child' that coincide \
         here by construction.",
    ));
    out.push(case(
        "proj_child_or_oob",
        cat,
        op_lit("proj_child_or", json!(9)),
        Some(json!([10])),
        engines_ok(Value::Null),
        "out-of-bounds child projection → null on engines; no \
         `proj_child_or` in term-interp.",
    ));
    out.push(case(
        "paren_single",
        cat,
        op("paren", vec![]),
        None,
        engines_ok(json!(7)),
        "EvalNode-gated: engines eval the single probe child; no `paren` \
         in term-interp.",
    ));
    out.push(case(
        "paren_multi",
        cat,
        op("paren", vec![]),
        None,
        engines_ok(json!([7, 8])),
        "multi-child `paren` → array of child values (engines only).",
    ));
    out.push(case(
        "typeof",
        cat,
        op("typeof", vec![lit(5)]),
        None,
        engines_ok(json!("number")),
        "term-interp has no `typeof` arm.",
    ));
    out.push(case(
        "tag_construct",
        cat,
        op("tag.construct", vec![lit("Num"), lit(json!({"x": 1}))]),
        None,
        engines_ok_ti_div(
            json!({"__tag__": "Num", "__registry__": "handl.canonical", "x": 1}),
            json!({"__tag__": "Num", "x": 1}),
        ),
        "engines stamp `__registry__: handl.canonical` alongside \
         `__tag__`; term-interp only inserts `__tag__` — SUSPECTED \
         DRIFT (registry provenance lost on the term path).",
    ));

    // ================= EvalNode-context ops =================
    let cat = "evalnode";
    out.push(case(
        "nullish_null",
        cat,
        op("nullish", vec![lit(Value::Null), lit(7)]),
        None,
        engines_ok(json!(7)),
        "term-interp has no `nullish` arm.",
    ));
    out.push(case(
        "nullish_some",
        cat,
        op(
            "nullish",
            vec![lit(json!({"_tag": "Some", "value": 5})), lit(7)],
        ),
        None,
        engines_ok(json!(5)),
        "",
    ));
    out.push(case(
        "nullish_value",
        cat,
        op("nullish", vec![lit(3), lit(7)]),
        None,
        engines_ok(json!(3)),
        "fixed in Phase B batch (a): a non-null, non-option left operand \
         passes through (`3 ?? 7` → 3), matching the `eval.nullish` \
         `tag`/`Choice` scheme. `null`/`None` still coalesce to `right` \
         and `Some(v)` unwraps.",
    ));
    out.push(case(
        "nullish_none",
        cat,
        op("nullish", vec![lit(json!({"_tag": "None"})), lit(7)]),
        None,
        engines_ok(json!(7)),
        "`None ?? 7` → 7 — the `None` arm was restored with the \
         pass-through fix (it had been dropped in the original \
         migration).",
    ));
    out.push(case(
        "opt_chain",
        cat,
        op("opt_chain", vec![lit(json!({"a": 5})), lit("a")]),
        None,
        engines_ok(json!({"_tag": "Some", "value": 5})),
        "engines wrap the projected field in Some; term-interp has no \
         `opt_chain` arm.",
    ));
    out.push(case(
        "opt_chain_null",
        cat,
        op("opt_chain", vec![lit(Value::Null), lit("a")]),
        None,
        engines_ok(json!({"_tag": "None"})),
        "",
    ));
    out.push(case(
        "index_args",
        cat,
        op("index", vec![lit(json!([10, 20, 30])), lit(1)]),
        None,
        engines_ok(json!(20)),
        "value-form `index` (two evaluated args).",
    ));
    out.push(case(
        "index_obj",
        cat,
        op("index", vec![lit(json!({"a": 5})), lit("a")]),
        None,
        engines_ok(json!(5)),
        "",
    ));
    out.push(case(
        "index_node_form",
        cat,
        op("index", vec![]),
        None,
        engines_ok(json!(20)),
        "EvalNode-gated form: `index` with <2 args reads \
         `payload.children` and evaluates the arena nodes — the three \
         engines provide `OpService::EvalNode`, the term interpreter \
         cannot.",
    ));

    // ================= loops =================
    let cat = "loop";
    out.push(case(
        "loop_left_exit",
        cat,
        TermData::loop_feedback(lit(eleft(42))),
        Some(json!(0)),
        all_ok(json!(42)),
        "body returns Left(v) on the first iteration on all four.",
    ));
    out.push(case(
        "loop_pair_exit",
        cat,
        TermData::loop_feedback(lit(pairv(9, 0))),
        Some(json!(0)),
        all_ok(json!(9)),
        "single-step mode: a plain pair result terminates with fst.",
    ));
    out.push(case(
        "loop_no_incoming",
        cat,
        TermData::loop_feedback(lit(eleft(42))),
        None,
        [
            Expect::Val(json!(42)),
            Expect::DivErr("underflow"),
            Expect::Val(json!(42)),
            Expect::Val(json!(42)),
        ],
        "SUSPECTED BUG (bytecode): a top-level `Loop` with no incoming \
         value underflows the operand stack — the `Push(Null); Pair` \
         prologue expects the loop input to already be on the stack. \
         tree-walk/clif/term-interp treat missing incoming as `null` and \
         evaluate the body.",
    ));
    out.push(case(
        "loop_countdown",
        cat,
        TermData::loop_feedback(op("countdown_step", vec![])),
        Some(pairv(42, 3)),
        engines_ok(json!(42)),
        "iterates Right(state-1) until 0, then Left(result=42).",
    ));
    out.push(case(
        "loop_while_step",
        cat,
        TermData::loop_feedback(op(
            "while_step",
            vec![op("is_pos", vec![]), op("dec", vec![])],
        )),
        Some(json!(3)),
        engines_ok(json!(0)),
        "cond/body sub-schemes re-evaluated per iteration on `current`; \
         term-interp lacks `while_step`.",
    ));
    out.push(case(
        "loop_step",
        cat,
        TermData::loop_feedback(op("loop_step", vec![lit(arrow(&["n"], 3))])),
        Some(json!(7)),
        engines_ok(json!(42)),
        "body arrow `n ~> {out: 42}` exits immediately; term-interp has \
         no `loop_step`.",
    ));
    out.push(case(
        "fold_step",
        cat,
        TermData::loop_feedback(op("fold_step", vec![lit(arrow(&["i", "s"], 4))])),
        Some(json!(7)),
        engines_ok(json!(0)),
        "step arrow `(i, s) ~> Left(0)` terminates on the first \
         iteration; term-interp has no `fold_step`.",
    ));

    // ================= Seq / cells =================
    let cat = "seq";
    out.push(case(
        "seq_two_steps",
        cat,
        TermData::seq(
            op("add", vec![op("id", vec![]), lit(1)]),
            op("add", vec![op("id", vec![]), lit(1)]),
        ),
        Some(json!(5)),
        all_ok(json!(7)),
        "`Seq{0}` pipes on all four.",
    ));
    out.push(case(
        "seq_const",
        cat,
        lit(2),
        Some(json!(1)),
        all_ok(json!(2)),
        "",
    ));
    out.push(case(
        "seq_axis1_dim2",
        cat,
        TermData::seq_axis(TermData::const_value(1), id_at(2), id_at(2)),
        Some(json!(7)),
        all_ok(cell2v(7, 7, 7)),
        "`Seq{axis:1}` lowers to `cell_comp_axis:2(axis, fst, snd)`; the \
         `id` arg sub-programs read the incoming `7` via `PushIncoming` \
         → `Cell2{7,7,7}` on all four.",
    ));
    out.push(case(
        "seq_axis2_dim3",
        cat,
        TermData::seq_axis(TermData::const_value(2), id_at(3), id_at(3)),
        Some(json!(7)),
        all_ok(cellnv(3, cell2v(7, 7, 7), cell2v(7, 7, 7), cell2v(7, 7, 7))),
        "same arg-position `id` incoming plumbing as `seq_axis1_dim2`, \
         at declared dimension 3.",
    ));
    out.push(case(
        "cell_comp_axis_op",
        cat,
        op("cell_comp_axis", vec![lit(1), lit(7), lit(7)]),
        None,
        all_ok(cell2v(7, 7, 7)),
        "explicit args — all four share `handl_value::cell_comp_axis` \
         (bare op name declares dimension 2).",
    ));
    out.push(case(
        "cell_comp_axis_dim3",
        cat,
        op("cell_comp_axis:3", vec![lit(2), lit(7), lit(7)]),
        None,
        all_ok(cellnv(3, cell2v(7, 7, 7), cell2v(7, 7, 7), cell2v(7, 7, 7))),
        "the `:<dim>` op-name suffix convention — parsed identically on \
         all four.",
    ));
    out.push(case(
        "cell_comp_axis_boundary",
        cat,
        op("cell_comp_axis", vec![lit(1), lit(7), lit(8)]),
        None,
        all_err("boundary mismatch"),
        "axis-1 paste on unequal values violates `tgt_1(a) = src_1(b)` \
         everywhere.",
    ));
    // `5 ∘₀ 6` at dimension 2: axis-0 pasting composes the cell
    // boundaries as 1-cell `Arrow{first,second}` records.
    let arrow_5_6 = || json!({"__tag__": "Arrow", "first": 5, "second": 6});
    let comp_5_6 =
        || json!({"__tag__": "Cell2", "__src__": arrow_5_6(), "__tgt__": arrow_5_6(), "value": 6});
    out.push(case(
        "compose_pipe",
        cat,
        op("compose", vec![]),
        Some(json!([5, {"axis": 0, "cell": 6}])),
        all_ok(comp_5_6()),
        "0-arg `compose` reads `incoming` (the diagram) via \
         `current_incoming` — `[5, {axis:0, cell:6}]` pastes `5 ∘₀ 6` → \
         `Cell2` whose boundaries are the 1-cell composite `Arrow{5,6}`.",
    ));
    out.push(case(
        "compose_args",
        cat,
        op("compose", vec![lit(5), lit(json!({"axis": 0, "cell": 6}))]),
        None,
        all_ok(comp_5_6()),
        "explicit step args — identical `cell_compose` on all four.",
    ));

    // ================= Handle / derived terms =================
    let cat = "derived";
    out.push(case(
        "handle_record",
        cat,
        TermData::handle(
            op_at_label("ping", "S"),
            "S",
            lit(json!({
                "grades": ["S"],
                "impls": {
                    "ping": term_value(op("add", vec![op("id", vec![]), lit(1)])),
                },
            })),
        ),
        Some(json!(5)),
        [
            Expect::Val(json!(6)),
            Expect::DivVal(json!({
                "grades": ["S"],
                "impls": {
                    "ping": term_value(op("add", vec![op("id", vec![]), lit(1)])),
                },
            })),
            Expect::DivVal(json!({
                "grades": ["S"],
                "impls": {
                    "ping": term_value(op("add", vec![op("id", vec![]), lit(1)])),
                },
            })),
            Expect::Val(json!(6)),
        ],
        "SUSPECTED DIVERGENCE (compiled backends): tree-walk and \
         term-interp dispatch `op ping` through the runtime handler \
         record's `impls` (→ 6); bytecode/clif *compile-time eliminate* \
         `Handle`, substituting the whole `impl_` term for the op — so \
         the op evaluates to the handler record itself. Two different \
         `Handle` semantics.",
    ));
    out.push(case(
        "handle_term_subst",
        cat,
        TermData::handle(
            op_at_label("ping", "S"),
            "S",
            op("add", vec![op("id", vec![]), lit(1)]),
        ),
        Some(json!(5)),
        [
            Expect::DivErr("unknown v2 native op"),
            Expect::Val(json!(6)),
            Expect::Val(json!(6)),
            Expect::DivErr("non-numeric"),
        ],
        "inverse of `handle_record`: when `impl_` is a bare *term* \
         (compile-time-substitution convention), bytecode/clif produce \
         6; tree-walk/term-interp evaluate `impl_` to the number `6`, \
         install it as the handler, find no `impls.ping`, and fail \
         dispatch — a real semantic split between the two `Handle` \
         models.",
    ));
    out.push(case(
        "child_role",
        cat,
        TermData::child("value"),
        None,
        tw_only(json!(33)),
        "`Term::Child` (named-role projection, ADR-0081): tree-walk \
         resolves via `lang.child_role_index_for`; bytecode/clif compile \
         it as `NotImplemented`; term-interp rejects the variant.",
    ));
    out.push(case(
        "iter",
        cat,
        TermData::iter(lit(10), op("is_pos", vec![]), op("dec", vec![]), lit(3)),
        None,
        tw_only(json!(0)),
        "`Term::Iter` bounded iteration: tree-walk only — \
         `NotImplemented` on both compiled backends, unsupported `Term` \
         in the interpreter.",
    ));
    out.push(case(
        "record_op",
        cat,
        op("record", vec![lit("a"), lit(1), lit("b"), lit(2)]),
        None,
        [
            Expect::Val(json!({"a": 1, "b": 2})),
            Expect::Unsupported,
            Expect::Unsupported,
            Expect::Val(json!({"a": 1, "b": 2})),
        ],
        "alternating key/value `record`: tree-walk residual arm and \
         term-interp's scheme form agree; bytecode/clif have no `record` \
         op at all — a coverage gap on the compiled backends.",
    ));

    // ================= Phase B batch (a): ops migrated to native_ops ====
    let cat = "batch-a";
    out.push(case(
        "field_access_obj",
        cat,
        op("field_access", vec![lit(json!({"a": 5, "b": 6})), lit("b")]),
        None,
        [
            Expect::Val(json!(6)),
            Expect::Val(json!(6)),
            Expect::Val(json!(6)),
            Expect::DivErr("field literal"),
        ],
        "args form (`eval.field-access`): engines resolve the field as \
         `payload.field → args[1] → lit`. term-interp's `field_access` \
         only takes the field as `lit` — a surface split, not a \
         semantics split.",
    ));
    out.push(case(
        "field_access_missing_key",
        cat,
        op("field_access", vec![lit(json!({"a": 5})), lit("zzz")]),
        None,
        [
            Expect::Val(Value::Null),
            Expect::Val(Value::Null),
            Expect::Val(Value::Null),
            Expect::DivErr("field literal"),
        ],
        "unlike `index`, a missing object key yields `null` (not an \
         error) — tree-walk semantics, now shared.",
    ));
    out.push(case(
        "field_access_arr",
        cat,
        op("field_access", vec![lit(json!([10, 20, 30])), lit(2)]),
        None,
        [
            Expect::Val(json!(30)),
            Expect::Val(json!(30)),
            Expect::Val(json!(30)),
            Expect::DivErr("field literal"),
        ],
        "array + integer field → element access.",
    ));
    out.push(case(
        "field_access_pair",
        cat,
        op("field_access", vec![lit(pairv(1, 2)), lit("first")]),
        None,
        [
            Expect::Val(json!(1)),
            Expect::Val(json!(1)),
            Expect::Val(json!(1)),
            Expect::DivErr("field literal"),
        ],
        "categorical `Pair` named views (`first`/`second`/`_tag`) — the \
         compiled backends gained these arms via unification; only \
         tree-walk had them before.",
    ));
    out.push(case(
        "field_access_str_join",
        cat,
        op("field_access", vec![lit("str"), lit("length")]),
        None,
        [
            Expect::Val(json!("str.length")),
            Expect::Val(json!("str.length")),
            Expect::Val(json!("str.length")),
            Expect::DivErr("field literal"),
        ],
        "string target joins `s.field` — the `str.length`-style host-path \
         convention.",
    ));
    out.push(case(
        "field_access_lit",
        cat,
        TermData::op(
            "field_access",
            vec![lit(json!({"a": 9}))],
            &ArrSource::Native,
            Some(json!("a")),
        ),
        None,
        [
            Expect::Val(json!(9)),
            Expect::DivErr("string/number"),
            Expect::DivErr("string/number"),
            Expect::Val(json!(9)),
        ],
        "lit form (arrow-lowered `op_native_lit` surface): tree-walk and \
         term-interp read the field from `lit`; bytecode/clif `NativeOp` \
         cannot carry `lit`, so the field arrives `null` — documented \
         plumbing gap, unchanged by the migration.",
    ));
    out.push(case(
        "proj_rest",
        cat,
        op("proj_rest", vec![]),
        None,
        engines_ok(json!([5, "x"])),
        "EvalNode-gated: evaluates children 1..N into an array — \
         `eval.call`'s variadic argument packing. term-interp has no \
         `proj_rest` arm.",
    ));
    out.push(case(
        "json_node_form",
        cat,
        op("json", vec![]),
        None,
        [
            Expect::Val(json!({"a": 5, "b": "x"})),
            Expect::Val(json!({"a": 5, "b": "x"})),
            Expect::Val(json!({"a": 5, "b": "x"})),
            Expect::DivErr("missing template literal"),
        ],
        "`eval.json` surface: `payload.children` = [template leaf, \
         args…]; the template is the leaf's raw payload (read via \
         `node_payload_at`, not evaluated) and `$i` holes take the \
         evaluated children. term-interp only knows the lit form.",
    ));
    out.push(case(
        "json_term_form",
        cat,
        TermData::op(
            "json",
            vec![lit(7)],
            &ArrSource::Native,
            Some(json!("{\"v\": $0}")),
        ),
        None,
        all_ok(json!({"v": 7})),
        "term form (arrow-lowered surface): `lit` is the template, args \
         are pre-evaluated — tree-walk/term-interp take it directly; \
         bytecode/clif drop `lit` at compile and reach the same result \
         via the node form (probe children carry an identical template \
         leaf).",
    ));
    out.push(case(
        "runtime_assert_pass",
        cat,
        op("runtime_assert", vec![]),
        None,
        engines_ok(json!(5)),
        "children [predicate, expr]: the `x ~> x > 0` closure applied to \
         5 returns truthy → pass-through. term-interp has no \
         `runtime_assert` arm.",
    ));
    out.push(case(
        "runtime_assert_fail",
        cat,
        op("runtime_assert", vec![]),
        None,
        [
            Expect::Err("assertion failed"),
            Expect::Err("assertion failed"),
            Expect::Err("assertion failed"),
            Expect::Unsupported,
        ],
        "predicate `x ~> x > 0` applied to -1 → false → LangError on all \
         three engines.",
    ));
    out.push(case(
        "runtime_assert_passthrough",
        cat,
        op("runtime_assert", vec![]),
        None,
        engines_ok(json!(8)),
        "a non-arrow predicate (a plain value — e.g. a type tag the \
         lowering wrapped anyway) passes the expr value through \
         unchecked.",
    ));

    // ================= Phase B batch (b) — scope/binding ops =============
    let cat = "batch-b";
    out.push(case(
        "assign_value",
        cat,
        op("assign", vec![]),
        None,
        engines_ok(json!(42)),
        "two-child `x = v` node form: `target`/`value` resolve by named \
         role (ADR-0081); the op evaluates `value` and returns it. \
         term-interp has no `assign` arm.",
    ));
    out.push(case(
        "assign_let",
        cat,
        op("assign", vec![]),
        None,
        engines_ok(json!(12)),
        "three-child `x = v in body`: the optional `body` role resolves \
         arity-aware (`child_role_index_for`), `x` binds in a pushed \
         scope, `body` evaluates under it (x + 7 = 12), scope pops.",
    ));
    out.push(case(
        "bind_scope",
        cat,
        op("bind", vec![]),
        None,
        engines_ok(json!(15)),
        "`bind` node form: payload `name` + children [value, body] — \
         value 5 binds `x` in a pushed scope, body `x + 10` → 15, scope \
         pops. term-interp has no `bind` arm.",
    ));
    out.push(case(
        "eval_block_bind",
        cat,
        op("eval_block", vec![]),
        None,
        engines_ok(json!(15)),
        "`do { y = 5; y + 10 }` shape: the two-child `assign` child binds \
         `y` into the block scope (`env_assign`), the trailing expr reads \
         it → 15. Leading leaf children are inert statements.",
    ));
    out.push(case(
        "eval_rec_converge",
        cat,
        op("eval_rec", vec![]),
        None,
        engines_ok(json!(11)),
        "`rec` region: `n = 1` seeds from the enclosing scope (Null), \
         defines `n` on pass 1, converges on pass 2 (`rec_value_eq`), \
         last child `n + 10` → 11.",
    ));
    out.push(case(
        "eval_rec_return",
        cat,
        op("eval_rec", vec![]),
        None,
        engines_ok(json!(99)),
        "a `return` child (shape declares `value` but no `target`) exits \
         the region early with its value — the fixpoint loop never \
         reaches the trailing leaf.",
    ));

    let cat = "batch-c";
    out.push(case(
        "array_literal",
        cat,
        op("array", vec![]),
        None,
        [
            Expect::Val(json!([10, 20, 30])),
            Expect::Val(json!([10, 20, 30])),
            Expect::Val(json!([10, 20, 30])),
            Expect::DivVal(json!([])),
        ],
        "`[a, b, c]` children form: each child evaluates to an element. \
         term-interp has an `array` arm but no AST arena — it folds the \
         op's (empty) arg list, so it pins `[]`.",
    ));
    out.push(case(
        "array_spread",
        cat,
        op("array", vec![]),
        None,
        [
            Expect::Val(json!([1, 7, 8, 9])),
            Expect::Val(json!([1, 7, 8, 9])),
            Expect::Val(json!([1, 7, 8, 9])),
            Expect::DivVal(json!([])),
        ],
        "`[1, ...r, 9]`: the middle child is `rest`-roled \
         (`pattern_role: rest`) and evaluates to `[7, 8]` — flattened \
         into the result. Same term-interp gap as `array_literal`.",
    ));
    out.push(case(
        "object_literal",
        cat,
        op("object", vec![]),
        None,
        engines_ok(json!({"a": 5, "b": 6})),
        "`{a: 5, b: 6}`: `entry`-roled children resolve `key`/`value` by \
         named role; the key child kind is declared literal for `key` \
         (`literal_children`), so its payload is the key text. \
         term-interp has no `object` arm.",
    ));
    out.push(case(
        "object_block",
        cat,
        op("object", vec![]),
        None,
        engines_ok(json!(4)),
        "ADR-0074 bare-block: every child is non-`entry` (bare leaves) — \
         block semantics evaluate each and return the last → 4.",
    ));
    out.push(case(
        "object_spread",
        cat,
        op("object", vec![]),
        None,
        engines_ok(json!({"p": 1, "q": 2})),
        "`{ ...src, q: 2 }`: a non-`entry` child in the `entries` range \
         is a spread — evaluated (`{p: 1}` via a nested `object` op) and \
         field-merged ahead of the `entry` child.",
    ));
    out.push(case(
        "closure_apply",
        cat,
        op("app", vec![op("closure", vec![]), lit(41)]),
        None,
        engines_ok(json!(51)),
        "`x ~> x + 10` applied to 41: the `closure` op resolves `params`/`\
         body` children by named role, `param_names` extracts `x`, the \
         minted `TaggedValue::Arrow` captures the (empty) env and `app` \
         applies it → 51. term-interp has neither op.",
    ));

    // ================= Phase B batch (d) — match / handle shells =======
    //
    // The shared `match_shell` (native_ops) resolves `scrutinee`/`block`
    // (`handle`: `target`/`impl`) on the probe root by named role —
    // arity-aware via `child_role_index_for` — iterates the block's arm
    // nodes (each declaring `pattern`/`body` roles), pushes a scope per
    // arm, runs the shared `try_pattern`, and evaluates the matched
    // body. term-interp has neither op (`unsupported op` → UNSUPPORTED).
    let cat = "batch-d";
    out.push(case(
        "match_literal_hit",
        cat,
        op("match", vec![]),
        None,
        engines_ok(json!(1)),
        "`match 5 { 5: 1 }` — literal pattern evaluates and compares → \
         body → 1.",
    ));
    out.push(case(
        "match_miss",
        cat,
        op("match", vec![]),
        None,
        [
            Expect::Err("no arm matched"),
            Expect::Err("no arm matched"),
            Expect::Err("no arm matched"),
            Expect::Unsupported,
        ],
        "`match 5 { 9: 1 }` — the literal misses; the shared shell's \
         documented no-match error fires on all three engines.",
    ));
    out.push(case(
        "match_bind",
        cat,
        op("match", vec![]),
        None,
        engines_ok(json!(5)),
        "`match 5 { x: x }` — the `var` binder binds the scrutinee in \
         the pushed arm scope; the body reads it via `proj_env`.",
    ));
    out.push(case(
        "match_multi_arm",
        cat,
        op("match", vec![]),
        None,
        engines_ok(json!(15)),
        "`match 5 { 9: 1, x: x + 10 }` — arm 1's literal misses (its \
         scope pops), arm 2's binder hits → 15. Covers miss fallthrough \
         + binding.",
    ));
    out.push(case(
        "match_rest",
        cat,
        op("match", vec![]),
        None,
        engines_ok(json!([2, 3])),
        "`match [1,2,3] { [a, ...rest]: rest }` — array destructuring \
         binds `a` = 1 and the rest tail `[2,3]` (`pattern_role: rest` \
         fallback on the `rest` kind).",
    ));
    out.push(case(
        "match_tag",
        cat,
        op("match", vec![]),
        None,
        engines_ok(json!(7)),
        "`match ok(7) { ok(v): v }` — the `call`-kind tag pattern \
         dispatches on `__tag__` and binds the payload field (ADR-0077).",
    ));
    out.push(case(
        "match_motive_arity",
        cat,
        op("match", vec![]),
        None,
        engines_ok(json!(9)),
        "root roles `[scrutinee, motive?, block]`: `block` resolves \
         arity-aware to position 2 with the optional motive slot \
         occupied (the motive node is never evaluated).",
    ));
    out.push(case(
        "match_no_arms",
        cat,
        op("match", vec![]),
        None,
        [
            Expect::Err("no arm matched"),
            Expect::Err("no arm matched"),
            Expect::Err("no arm matched"),
            Expect::Unsupported,
        ],
        "two-node probe (`[scrutinee, block]`, arity 2) on the \
         optional-motive shape: `block` resolves arity-aware to \
         position 1 (motive absent); the empty block falls through to \
         the no-match error.",
    ));
    out.push(case(
        "handle_clause",
        cat,
        op("handle", vec![]),
        None,
        engines_ok(json!(6)),
        "`handle 5 { x: x + 1 }` — `target`/`impl` resolve by named \
         role; the binder clause matches → 6.",
    ));
    out.push(case(
        "handle_no_clause",
        cat,
        op("handle", vec![]),
        None,
        engines_ok(json!(5)),
        "`handle 5 { 9: 1 }` — no clause matches; the handled value \
         passes through unchanged (the documented `handle` default — \
         not an error).",
    ));
    out.push(case(
        "handle_tag",
        cat,
        op("handle", vec![]),
        None,
        engines_ok(json!("bad")),
        "`handle err(\"bad\") { err(e): e }` — clause tag-dispatch on \
         `__tag__` binds the payload field.",
    ));

    // Wire up the probe nodes every case needs (indices referenced by
    // `arrow`/`arrow_pat` body fields):
    //
    //   0 — `fn.inc10`  : `x ~> x + 10`
    //   1 — `fn.add100` : `x ~> x + 100`
    //   2 — `fn.add_ab` : `[a, b] ~> a + b`  (array-pattern)
    //   3 — `fn.out42`  : `n ~> {"out": 42}`
    //   4 — `fn.left0`  : `(i, s) ~> Left(0)`
    for case in &mut out {
        case.nodes = vec![
            schemed("fn.inc10", inc10_scheme()),
            schemed("fn.add100", add100_scheme()),
            schemed(
                "fn.add_ab",
                op(
                    "add",
                    vec![
                        op_lit("proj_env", json!("a")),
                        op_lit("proj_env", json!("b")),
                    ],
                ),
            ),
            schemed("fn.out42", lit(json!({"out": 42}))),
            schemed("fn.left0", lit(eleft(0))),
        ];
        // `paren`/`proj_child`/`index`-node-form cases address children
        // positionally — replace the shared fn-table with the case's own
        // leaves when it declared none.
        if case.name == "proj_child" || case.name == "proj_child_or_oob" {
            case.nodes = vec![leaf(json!(10)), leaf(json!(20)), leaf(json!(30))];
        }
        if case.name == "paren_single" {
            case.nodes = vec![leaf(json!(7))];
        }
        if case.name == "paren_multi" {
            case.nodes = vec![leaf(json!(7)), leaf(json!(8))];
        }
        if case.name == "index_node_form" {
            case.nodes = vec![leaf(json!([10, 20, 30])), leaf(json!(1))];
        }
        if case.name == "child_role" {
            case.nodes = vec![leaf(json!(33))];
            case.roles = vec!["value"];
        }
        if case.name == "proj_rest" {
            // children[1..] are the evaluated varargs — child 0 is
            // skipped (call position).
            case.nodes = vec![leaf(json!(0)), leaf(json!(5)), leaf(json!("x"))];
        }
        if case.name == "json_node_form" {
            // children[0] is the template leaf (raw payload — NOT
            // evaluated); children[1..] evaluate to the `$i` arguments.
            case.nodes = vec![
                leaf(json!("{\"a\": $0, \"b\": $1}")),
                leaf(json!(5)),
                leaf(json!("x")),
            ];
        }
        if case.name == "json_term_form" {
            // bytecode/clif drop the `lit` template at compile — the same
            // template is also provided as a leaf so the node form
            // produces the identical value.
            case.nodes = vec![leaf(json!("{\"v\": $0}")), leaf(json!(7))];
        }
        if case.name == "runtime_assert_pass" || case.name == "runtime_assert_fail" {
            // children[0] = predicate producer (evaluates to the arrow
            // `x ~> x > 0`, whose body is arena index 2);
            // children[1] = the expr under test.
            let expr = if case.name == "runtime_assert_pass" {
                json!(5)
            } else {
                json!(-1)
            };
            case.nodes = vec![
                schemed("fn.pred_wrap", lit(arrow(&["x"], 2))),
                leaf(expr),
                schemed(
                    "fn.gt0",
                    op("gt", vec![op_lit("proj_env", json!("x")), lit(0)]),
                ),
            ];
        }
        if case.name == "runtime_assert_passthrough" {
            // children[0] evaluates to a non-arrow value — the shared op
            // skips application and passes the expr value through.
            case.nodes = vec![leaf(json!(true)), leaf(json!(8))];
        }
        if case.name == "assign_value" {
            // children [target, value] — `target`/`value` roles on the
            // probe root.
            case.nodes = vec![leaf(json!("x")), leaf(json!(42))];
            case.roles = vec!["target", "value"];
        }
        if case.name == "assign_let" {
            // children [target, value, body] — `body` declared on the
            // probe root so its presence resolves arity-aware.
            case.nodes = vec![
                leaf(json!("x")),
                leaf(json!(5)),
                schemed(
                    "fn.body_x7",
                    op("add", vec![op_lit("proj_env", json!("x")), lit(7)]),
                ),
            ];
            case.roles = vec!["target", "value", "body"];
        }
        if case.name == "bind_scope" {
            // payload `name` + children [value, body].
            case.nodes = vec![
                leaf(json!(5)),
                schemed(
                    "fn.body_x10",
                    op("add", vec![op_lit("proj_env", json!("x")), lit(10)]),
                ),
            ];
            case.root_payload = Some(json!({"name": "x"}));
        }
        if case.name == "eval_block_bind" {
            // children [y-leaf, 5-leaf, assign{y:0,v:1}, last-expr] — the
            // assign child (kind `pb.assign`) declares `target`/`value`.
            case.nodes = vec![
                leaf(json!("y")),
                leaf(json!(5)),
                ProbeNode {
                    kind: "pb.assign",
                    payload: json!({"children": [0, 1]}),
                    roles: vec![("target", false), ("value", false)],
                    ..leaf(Value::Null)
                },
                schemed(
                    "fn.block_last",
                    op("add", vec![op_lit("proj_env", json!("y")), lit(10)]),
                ),
            ];
        }
        if case.name == "eval_rec_converge" {
            // children [n-leaf, 1-leaf, assign{n:0,v:1}, last-expr].
            case.nodes = vec![
                leaf(json!("n")),
                leaf(json!(1)),
                ProbeNode {
                    kind: "pr.assign",
                    payload: json!({"children": [0, 1]}),
                    roles: vec![("target", false), ("value", false)],
                    ..leaf(Value::Null)
                },
                schemed(
                    "fn.rec_last",
                    op("add", vec![op_lit("proj_env", json!("n")), lit(10)]),
                ),
            ];
        }
        if case.name == "eval_rec_return" {
            // children [n-leaf, 1-leaf, assign{n:0,v:1}, return{4}, 99]
            // — the `pr.ret` kind declares `value` only (a return shape).
            case.nodes = vec![
                leaf(json!("n")),
                leaf(json!(1)),
                ProbeNode {
                    kind: "pr.assign",
                    payload: json!({"children": [0, 1]}),
                    roles: vec![("target", false), ("value", false)],
                    ..leaf(Value::Null)
                },
                ProbeNode {
                    kind: "pr.ret",
                    payload: json!({"children": [4]}),
                    roles: vec![("value", false)],
                    ..leaf(Value::Null)
                },
                leaf(json!(99)),
            ];
        }
        if case.name == "array_literal" {
            // children [10, 20, 30] — bare leaves evaluate to themselves.
            case.nodes = vec![leaf(json!(10)), leaf(json!(20)), leaf(json!(30))];
        }
        if case.name == "array_spread" {
            // children [1, rest-node, 9] — the `pb.rest` kind declares
            // `pattern_role: rest`; its scheme evaluates to `[7, 8]` and
            // the `array` op flattens it.
            case.nodes = vec![
                leaf(json!(1)),
                ProbeNode {
                    kind: "pb.rest",
                    scheme: Some(lit(json!([7, 8]))),
                    pattern_role: Some("rest"),
                    ..leaf(Value::Null)
                },
                leaf(json!(9)),
            ];
        }
        if case.name == "object_literal" {
            // children [k, v, k, v, entry{0,1}, entry{2,3}] — the leading
            // leaves occupy dummy roles so `entries` (index 4) ranges
            // over exactly the two `entry` nodes.
            case.nodes = vec![
                leaf(json!("a")),
                leaf(json!(5)),
                leaf(json!("b")),
                leaf(json!(6)),
                entry_node(0, 1),
                entry_node(2, 3),
            ];
            case.roles = vec!["k.a", "v.a", "k.b", "v.b", "entries"];
        }
        if case.name == "object_block" {
            // children [3, 4] — all non-`entry` leaves → bare-block
            // semantics (evaluate each, return the last).
            case.nodes = vec![leaf(json!(3)), leaf(json!(4))];
        }
        if case.name == "object_spread" {
            // children [k, v, spread-src, entry{0,1}] — `entries` (index
            // 2) covers [spread, entry]. The spread child evaluates to
            // `{p: 1, q: 1}` via a nested `object` op on its rich
            // `entries` payload; the `entry` child then overwrites `q`.
            case.nodes = vec![
                leaf(json!("q")),
                leaf(json!(2)),
                ProbeNode {
                    kind: "pb.obj",
                    payload: json!({"entries": [{"key": "p", "value": 1}, {"key": "q", "value": 1}]}),
                    scheme: Some(op("object", vec![])),
                    ..leaf(Value::Null)
                },
                entry_node(0, 1),
            ];
            case.roles = vec!["k", "v", "entries"];
        }
        if case.name == "closure_apply" {
            // children [params-leaf("x"), body] — `params`/`body` roles
            // on the probe root resolve positions 0/1 (ADR-0081). The
            // params leaf is a single-param node (no variadic role →
            // `param_names` returns `["x"]`).
            case.nodes = vec![leaf(json!("x")), schemed("fn.body_x10", inc10_scheme())];
            case.roles = vec!["params", "body"];
        }
        // ---- batch-d: `match`/`handle` shells ------------------------
        // Root roles resolve `scrutinee`/`block` (`handle`: `target`/
        // `impl`); `pb.block` children are arm arena indices; `pb.arm`
        // declares `pattern`/`body`. Pattern nodes use the canonical
        // role-less kinds (`var`/`array`/`rest`/`call`/`lit`) so both
        // the declared-role and kind-fallback paths are exercised.
        if case.name == "match_literal_hit" || case.name == "match_miss" {
            // children [scrutinee=5, block]; arm { lit-pattern, body }.
            let pat = if case.name == "match_miss" {
                json!(9)
            } else {
                json!(5)
            };
            case.nodes = vec![
                leaf(json!(5)),
                block_node(&[2]),
                arm_node(3, 4),
                leaf(pat),
                leaf(json!(1)),
            ];
            case.roles = vec!["scrutinee", "block"];
        }
        if case.name == "match_bind" {
            // children [scrutinee=5, block]; arm { var "x", proj_env x }.
            case.nodes = vec![
                leaf(json!(5)),
                block_node(&[2]),
                arm_node(3, 4),
                var_node("x"),
                schemed("fn.m_x", op_lit("proj_env", json!("x"))),
            ];
            case.roles = vec!["scrutinee", "block"];
        }
        if case.name == "match_multi_arm" {
            // children [5, block{2,5}]; arm2 { lit 9, 1 } misses;
            // arm5 { var x, x + 10 } hits → 15.
            case.nodes = vec![
                leaf(json!(5)),
                block_node(&[2, 5]),
                arm_node(3, 4),
                leaf(json!(9)),
                leaf(json!(1)),
                arm_node(6, 7),
                var_node("x"),
                schemed(
                    "fn.m_x10",
                    op("add", vec![op_lit("proj_env", json!("x")), lit(10)]),
                ),
            ];
            case.roles = vec!["scrutinee", "block"];
        }
        if case.name == "match_rest" {
            // `match [1,2,3] { [a, ...rest]: rest }` — array pattern at
            // 3 over positional var(4) + rest node(5)→var(6); body(7)
            // reads `rest`.
            case.nodes = vec![
                leaf(json!([1, 2, 3])),
                block_node(&[2]),
                arm_node(3, 7),
                ProbeNode {
                    kind: "array",
                    payload: json!({"children": [4, 5]}),
                    ..leaf(Value::Null)
                },
                var_node("a"),
                ProbeNode {
                    kind: "rest",
                    payload: json!({"children": [6]}),
                    ..leaf(Value::Null)
                },
                var_node("rest"),
                schemed("fn.m_rest", op_lit("proj_env", json!("rest"))),
            ];
            case.roles = vec!["scrutinee", "block"];
        }
        if case.name == "match_tag" {
            // `match ok(7) { ok(v): v }` — `call` tag pattern at 3:
            // callee var(4)="ok", binder var(5)="v"; body(6) reads `v`.
            case.nodes = vec![
                leaf(json!({"__tag__": "ok", "value": 7})),
                block_node(&[2]),
                arm_node(3, 6),
                ProbeNode {
                    kind: "call",
                    payload: json!({"children": [4, 5]}),
                    ..leaf(Value::Null)
                },
                var_node("ok"),
                var_node("v"),
                schemed("fn.m_v", op_lit("proj_env", json!("v"))),
            ];
            case.roles = vec!["scrutinee", "block"];
        }
        if case.name == "match_motive_arity" {
            // Root `[scrutinee, motive?, block]` (arity 6 → motive
            // occupied): scrutinee 0 = 5; motive 1 is never evaluated;
            // block 2 → arm { wildcard `_`, 9 }.
            case.nodes = vec![
                leaf(json!(5)),
                leaf(json!("motive-slot")),
                block_node(&[3]),
                arm_node(4, 5),
                var_node("_"),
                leaf(json!(9)),
            ];
            case.roles = vec!["scrutinee", "motive", "block"];
            case.optional_roles = vec!["motive"];
        }
        if case.name == "match_no_arms" {
            // Arity-2 probe on the same optional-motive shape → `block`
            // resolves to position 1 (motive absent); the empty block
            // yields the no-match error.
            case.nodes = vec![leaf(json!(5)), block_node(&[])];
            case.roles = vec!["scrutinee", "motive", "block"];
            case.optional_roles = vec!["motive"];
        }
        if case.name == "handle_clause" {
            // `handle 5 { x: x + 1 }` — `target`/`impl` roles.
            case.nodes = vec![
                leaf(json!(5)),
                block_node(&[2]),
                arm_node(3, 4),
                var_node("x"),
                schemed(
                    "fn.h_x1",
                    op("add", vec![op_lit("proj_env", json!("x")), lit(1)]),
                ),
            ];
            case.roles = vec!["target", "impl"];
        }
        if case.name == "handle_no_clause" {
            // `handle 5 { 9: 1 }` — literal clause misses → pass-through.
            case.nodes = vec![
                leaf(json!(5)),
                block_node(&[2]),
                arm_node(3, 4),
                leaf(json!(9)),
                leaf(json!(1)),
            ];
            case.roles = vec!["target", "impl"];
        }
        if case.name == "handle_tag" {
            // `handle err("bad") { err(e): e }` — `call` clause pattern.
            case.nodes = vec![
                leaf(json!({"__tag__": "err", "value": "bad"})),
                block_node(&[2]),
                arm_node(3, 6),
                ProbeNode {
                    kind: "call",
                    payload: json!({"children": [4, 5]}),
                    ..leaf(Value::Null)
                },
                var_node("err"),
                var_node("e"),
                schemed("fn.h_e", op_lit("proj_env", json!("e"))),
            ];
            case.roles = vec!["target", "impl"];
        }
    }

    out
}
