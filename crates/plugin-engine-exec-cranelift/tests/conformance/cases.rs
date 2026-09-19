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

/// A closure value `{"__arrow__": true, "params": [...], "body": <arena
/// idx>, "env": {}}` — the shape all three `EvalEngine`s apply via
/// `apply_app`. `body` indexes a [`ProbeNode`] carrying the body scheme.
fn arrow(params: &[&str], body: usize) -> Value {
    let ps: Vec<Value> = params.iter().map(|p| json!(p)).collect();
    json!({ "__arrow__": true, "params": ps, "body": body, "env": {} })
}

/// Same, but an array-pattern arrow (`[a, b] ~> body` destructures the
/// single array argument by position).
fn arrow_pat(params: &[&str], body: usize) -> Value {
    let ps: Vec<Value> = params.iter().map(|p| json!(p)).collect();
    json!({ "__arrow__": true, "params": ps, "body": body, "env": {}, "array_pattern": true })
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
    }
}

/// Probe node carrying its own eval scheme (closure bodies).
fn schemed(kind: &'static str, scheme: TermData) -> ProbeNode {
    ProbeNode {
        kind,
        payload: Value::Null,
        scheme: Some(scheme),
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
    [Expect::Err(sub), Expect::Err(sub), Expect::Err(sub), Expect::Err(sub)]
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
        expect,
        note,
    }
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// `x ~> x + 10` body scheme for `arrow(&["x"], 0)` fixtures.
fn inc10_scheme() -> TermData {
    op(
        "add",
        vec![op_lit("proj_env", json!("x")), lit(10)],
    )
}

/// `x ~> x + 100`.
fn add100_scheme() -> TermData {
    op(
        "add",
        vec![op_lit("proj_env", json!("x")), lit(100)],
    )
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
        [
            Expect::Val(json!(7)),
            Expect::DivVal(json!(1)),
            Expect::Val(json!(7)),
            Expect::DivVal(json!(5)),
        ],
        "SUSPECTED DRIFT — three different n-ary conventions: \
         tree-walk/cranelift take args[0]-args[1] via native_ops::bin_num \
         (10-3=7); bytecode's typed `Sub` instruction pops the *last two* \
         stack operands (3-2=1); term-interp folds all args left \
         (10-3-2=5).",
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
        op(
            "number_add",
            vec![lit(num_tag(2)), lit(num_tag(3))],
        ),
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
        [
            Expect::Val(json!(true)),
            Expect::DivVal(json!(false)),
            Expect::Val(json!(true)),
            Expect::DivVal(json!(false)),
        ],
        "three conventions: native_ops `and` reads args[0] && args[1] \
         (true); bytecode `Arith(And)` pops the last two operands \
         (true && false = false); term-interp folds all args (false). \
         SUSPECTED DRIFT.",
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
        [
            Expect::Val(json!(false)),
            Expect::DivVal(json!(true)),
            Expect::Val(json!(false)),
            Expect::DivVal(json!(true)),
        ],
        "three conventions: native_ops `or` reads args[0] || args[1] \
         (false); bytecode `Arith(Or)` pops the last two operands \
         (false || true = true); term-interp folds all args (true). \
         SUSPECTED DRIFT.",
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
        "MASKED drift (bytecode): `id` in arg position 1 pops the `1` \
         arg off the operand stack rather than reading `incoming` — the \
         corrupting read is invisible only because `id(1)=1`; the real \
         incoming `7` is then consumed by `add` itself. See \
         `seq_inc_later_arg` for the unmasked variant.",
    ));
    out.push(case(
        "seq_inc_later_arg",
        cat,
        op("sub", vec![lit(10), op("inc", vec![])]),
        Some(json!(7)),
        [
            Expect::Val(json!(2)),
            Expect::DivVal(json!(-4)),
            Expect::Val(json!(2)),
            Expect::Unsupported,
        ],
        "SUSPECTED BUG (bytecode): `inc` in arg position 1 pops the `10` \
         arg (not the Seq predecessor `7`), then `sub` pops `7` and `11` \
         → 7-11 = -4. tree-walk/cranelift evaluate `inc` against \
         `incoming` → 10 - 8 = 2. term-interp has no `inc` op.",
    ));
    out.push(case(
        "seq_first_incoming",
        cat,
        op("first", vec![lit(arrow(&["x"], 0))]),
        Some(pairv(1, 99)),
        [
            Expect::Val(pairv(11, 99)),
            Expect::DivErr("not a pair"),
            Expect::Val(pairv(11, 99)),
            Expect::Unsupported,
        ],
        "SUSPECTED BUG (bytecode): `first` consumes `incoming` but is not \
         in `op_uses_incoming` — `NativeOp` passes `current_incoming` \
         (the program-level incoming, `None`) instead of the Seq \
         predecessor's pair. term-interp lacks the `first` op.",
    ));
    out.push(case(
        "seq_app_incoming",
        cat,
        op("app", vec![lit(arrow(&["x"], 0))]),
        Some(json!(41)),
        [
            Expect::Val(json!(51)),
            Expect::DivErr("number"),
            Expect::Val(json!(51)),
            Expect::Unsupported,
        ],
        "SUSPECTED BUG (bytecode): `app` gets `incoming=None` (not in \
         `op_uses_incoming`), so the arrow is applied to `null` and its \
         `x+10` body fails. tree-walk/clif apply it to 41 → 51.",
    ));
    out.push(case(
        "seq_sum_incoming",
        cat,
        op("sum", vec![lit(arrow(&["x"], 0)), lit(arrow(&["x"], 1))]),
        Some(eleft(5)),
        [
            Expect::Val(eleft(15)),
            Expect::DivErr("not an either"),
            Expect::Val(eleft(15)),
            Expect::Unsupported,
        ],
        "SUSPECTED BUG (bytecode): same incoming-plumbing gap as \
         `seq_first_incoming` — `sum` cannot see the Seq predecessor.",
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
            vec![
                lit(arrow(&["x"], 0)),
                lit(arrow(&["x"], 1)),
                lit(eleft(5)),
            ],
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
        [
            Expect::Unsupported,
            Expect::Unsupported,
            Expect::Unsupported,
            Expect::Val(eleft(7)),
        ],
        "inverse gap: `pair_tag` exists only in the term-interpreter's \
         op table (used by the `if` lowering) — the engines never \
         implement it.",
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
        engines_ok(json!(7)),
        "SUSPECTED BUG (native_ops, all engines agree): `3 ?? 7` yields \
         7 — a non-null, non-`Some` left operand falls into the final \
         `else` and returns `right`. Consensus-correct by construction \
         (shared op table), likely wrong semantics — expected `left` = 3.",
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
        TermData::loop_feedback(op(
            "fold_step",
            vec![lit(arrow(&["i", "s"], 4))],
        )),
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
        TermData::seq_axis(
            TermData::const_value(1),
            id_at(2),
            id_at(2),
        ),
        Some(json!(7)),
        [
            Expect::Val(cell2v(7, 7, 7)),
            Expect::DivErr("underflow"),
            Expect::Val(cell2v(7, 7, 7)),
            Expect::Val(cell2v(7, 7, 7)),
        ],
        "SUSPECTED BUG (bytecode): `Seq{axis:1}` lowers to \
         `cell_comp_axis:2(axis, fst, snd)` and the `id` arg sub-programs \
         corrupt the operand stack (each pops whatever the previous arg \
         pushed) → `stack underflow on NativeOp`. tree-walk/clif/\
         term-interp evaluate the operands against the Seq incoming `7` \
         → `Cell2{7,7,7}`.",
    ));
    out.push(case(
        "seq_axis2_dim3",
        cat,
        TermData::seq_axis(
            TermData::const_value(2),
            id_at(3),
            id_at(3),
        ),
        Some(json!(7)),
        [
            Expect::Val(cellnv(3, cell2v(7, 7, 7), cell2v(7, 7, 7), cell2v(7, 7, 7))),
            Expect::DivErr("underflow"),
            Expect::Val(cellnv(3, cell2v(7, 7, 7), cell2v(7, 7, 7), cell2v(7, 7, 7))),
            Expect::Val(cellnv(3, cell2v(7, 7, 7), cell2v(7, 7, 7), cell2v(7, 7, 7))),
        ],
        "SUSPECTED BUG (bytecode): same arg-position `id` stack \
         corruption as `seq_axis1_dim2`, at declared dimension 3.",
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
        op(
            "cell_comp_axis:3",
            vec![lit(2), lit(7), lit(7)],
        ),
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
    let arrow_5_6 = || {
        json!({"__tag__": "Arrow", "first": 5, "second": 6})
    };
    let comp_5_6 = || {
        json!({"__tag__": "Cell2", "__src__": arrow_5_6(), "__tgt__": arrow_5_6(), "value": 6})
    };
    out.push(case(
        "compose_pipe",
        cat,
        op("compose", vec![]),
        Some(json!([5, {"axis": 0, "cell": 6}])),
        [
            Expect::Val(comp_5_6()),
            Expect::DivErr("array"),
            Expect::Val(comp_5_6()),
            Expect::Val(comp_5_6()),
        ],
        "SUSPECTED BUG (bytecode): 0-arg `compose` needs `incoming` (the \
         diagram) but is not in `op_uses_incoming` → gets \
         `current_incoming` (`None`) → 'must be an array' error. Diagram \
         `[5, {axis:0, cell:6}]` pastes `5 ∘₀ 6` → `Cell2` whose \
         boundaries are the 1-cell composite `Arrow{5,6}`.",
    ));
    out.push(case(
        "compose_args",
        cat,
        op(
            "compose",
            vec![lit(5), lit(json!({"axis": 0, "cell": 6}))],
        ),
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
        TermData::iter(
            lit(10),
            op("is_pos", vec![]),
            op("dec", vec![]),
            lit(3),
        ),
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
    }

    out
}
