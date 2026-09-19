//! Cross-interpreter conformance harness (Phase A of the evaluator
//! unification effort).
//!
//! Runs *identical* [`TermData`] inputs through all four evaluators —
//! the tree-walk, bytecode, and Cranelift `EvalEngine`s plus the pure
//! `plugin_engine::term_interpreter` — and reports a per-case status
//! matrix: `PASS` / `DIVERGENT` / `UNSUPPORTED` / `FAIL`.
//!
//! # Mechanics
//!
//! The three `EvalEngine`s evaluate an `Ast` whose nodes carry
//! `lang.eval[node.kind]` schemes, so each case is wrapped in a
//! single-node probe language:
//!
//! ```text
//!     nodes[0..n]   — optional probe children (EvalNode-context ops)
//!     probe (root)  — scheme = `Seq{0, Arr(input), body}`
//! ```
//!
//! `Seq{0, Arr(input), body}` pipes `input` into `body` on every
//! interpreter, so ops that consume `incoming` (`id`, `inc`,
//! `countdown_step`, `first`, `sum`, …) see the same value everywhere.
//! When `input` is `None` the body runs bare. The pure term
//! interpreter receives the identical `Term` via
//! [`plugin_engine::term_interpreter::interpret`].
//!
//! `body` may address probe children positionally via `proj_child` /
//! `paren` / `index` node-form, or by named role via `Term::Child` —
//! declare roles on the `probe` kind through [`Case::roles`].
//!
//! # Status vocabulary
//!
//! - `PASS` — the interpreter produced the case's consensus outcome.
//! - `DIVERGENT` — a *pinned* documented divergence: the interpreter
//!   produced exactly the expected different value/error.
//! - `UNSUPPORTED` — the interpreter lacks the capability and reported
//!   an error (the actual error is captured in the report).
//! - `FAIL` — anything else: an unexpected value, an unexpected error,
//!   or a panic. `FAIL` is what the test asserts against — it means
//!   "undocumented drift or a regression".
//!
//! Every divergence in the matrix is therefore *asserted*: if an engine
//! drifts further (or a divergence is silently fixed), the suite fails
//! and forces an explicit re-baseline.

use std::panic::{AssertUnwindSafe, catch_unwind};

use handl_language::{Ast, AstNode, AstShapeDef, ChildRoleSpec, EvalSchemeDef, Language};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use plugin_engine::TermData;
use serde_json::{Value, json};

/// The four interpretation paths, in report column order.
pub const INTERPRETERS: [&str; 4] = ["tree-walk", "bytecode", "cranelift", "term-interp"];

// ---------------------------------------------------------------------------
// Case description
// ---------------------------------------------------------------------------

/// An extra arena node the probe root's `children` list can reference.
///
/// `scheme` is optional: a node without a scheme falls back to returning
/// its payload verbatim (all engines treat scheme-less nodes as literal
/// leaves), which is what `proj_child`/`paren`/`index` fixtures want.
pub struct ProbeNode {
    /// Node kind — must be unique within the case when `scheme` is set.
    pub kind: &'static str,
    /// Node payload (leaf value when `scheme` is `None`).
    pub payload: Value,
    /// Optional eval scheme for the node's kind.
    pub scheme: Option<TermData>,
    /// Named child roles declared on this node's kind (ADR-0081) as
    /// `(role, optional)` pairs — needed when an op inspects the *child's*
    /// shape (`eval_block`/`eval_rec` binding detection, `assign`'s
    /// optional `body`).
    pub roles: Vec<(&'static str, bool)>,
    /// Subset of `roles` declared **variadic** (ADR-0081) — e.g. the
    /// params-list/`elements` role a `closure`/`array` node's children
    /// occupy. Feeds `has_variadic_role`, which `param_names` and
    /// `is_array_pattern` dispatch on.
    pub variadic_roles: Vec<&'static str>,
    /// `AstShapeDef::pattern_role` tag for this node's kind (ADR-0081) —
    /// `rest` (spread element), `entry` (object field), `sequence`
    /// (array-destructuring params), …, read by the constructor ops via
    /// `pattern_role_at`.
    pub pattern_role: Option<&'static str>,
    /// `AstShapeDef::literal_children` entries as `(role, kinds)` pairs
    /// (ADR-0081) — e.g. `("key", ["lit"])` on an `entry` shape marks the
    /// key child's payload as the literal key text rather than an
    /// expression to evaluate.
    pub literal_children: Vec<(&'static str, Vec<&'static str>)>,
}

/// A single conformance case: one [`TermData`] body run on all four
/// interpreters against the same optional `input`.
pub struct Case {
    /// Short unique name — the report row key.
    pub name: &'static str,
    /// Coverage bucket (arithmetic, products, env, loops, …).
    pub category: &'static str,
    /// The term under test.
    pub body: TermData,
    /// Incoming value: `Some(v)` wraps the body as
    /// `Seq{0, Arr(v), body}`; `None` runs the body bare.
    pub input: Option<Value>,
    /// Extra arena nodes for EvalNode-context ops (`proj_child`,
    /// `paren`, node-form `index`, closure bodies, …).
    pub nodes: Vec<ProbeNode>,
    /// Named child roles declared on the probe root kind (ADR-0081) —
    /// needed by `Term::Child { role }` cases.
    pub roles: Vec<&'static str>,
    /// Extra fields merged into the probe root's payload object (alongside
    /// the `children` array the harness fills in) — needed by ops that
    /// read a named payload field off the current node (`bind`'s `name`).
    pub root_payload: Option<Value>,
    /// Per-interpreter expectation, in [`INTERPRETERS`] order.
    pub expect: [Expect; 4],
    /// Free-form note printed beside the row — document *why* a
    /// divergence or gap exists.
    pub note: &'static str,
}

/// What a case expects one interpreter to produce.
#[derive(Clone)]
pub enum Expect {
    /// Consensus `Ok` value → `PASS`.
    Val(Value),
    /// Consensus `Err` whose message contains this substring → `PASS`.
    /// Use when *every* interpreter is expected to reject the case.
    Err(&'static str),
    /// Documented divergence: a pinned `Ok` value that differs from the
    /// consensus → `DIVERGENT`.
    DivVal(Value),
    /// Documented divergence: a pinned `Err` (substring) where the
    /// consensus is `Ok` → `DIVERGENT`.
    DivErr(&'static str),
    /// Capability gap: the interpreter is expected to `Err` (any
    /// message) → `UNSUPPORTED`.
    Unsupported,
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// What an interpreter actually produced.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// `Ok` value.
    Val(Value),
    /// `Err` with the rendered message.
    Err(String),
    /// The interpreter panicked (caught via `catch_unwind`); always a
    /// defect — never an accepted status.
    Panic(String),
}

impl Outcome {
    /// One-line rendering for the report.
    pub fn render(&self) -> String {
        match self {
            Outcome::Val(v) => {
                let s = v.to_string();
                if s.chars().count() > 60 {
                    format!("{}…", s.chars().take(59).collect::<String>())
                } else {
                    s
                }
            }
            Outcome::Err(e) => format!("Err({e})"),
            Outcome::Panic(e) => format!("PANIC({e})"),
        }
    }
}

/// Report status for one (case, interpreter) cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Consensus outcome produced.
    Pass,
    /// Pinned documented divergence produced.
    Divergent,
    /// Capability gap — interpreter errored as expected.
    Unsupported,
    /// Expectation mismatch — undocumented drift.
    Fail,
}

/// One report cell: the expectation-derived status plus the actual
/// outcome (kept verbatim for the detail dump).
pub struct Cell {
    pub status: Status,
    pub outcome: Outcome,
}

/// A full report row.
pub struct Row {
    pub name: &'static str,
    pub category: &'static str,
    pub note: &'static str,
    pub cells: Vec<Cell>,
}

// ---------------------------------------------------------------------------
// Probe language / AST construction
// ---------------------------------------------------------------------------

/// The scheme actually under test: `Seq{0, Arr(input), body}` when the
/// case carries an input, else the body itself.
pub fn tested_term(case: &Case) -> TermData {
    match &case.input {
        Some(v) => TermData::seq(TermData::arr(v.clone()), case.body.clone()),
        None => case.body.clone(),
    }
}

/// Build the probe `Language` + `Ast` for a case.
///
/// The `Ast` holds `case.nodes` first, then the `probe` root last
/// (engines evaluate the arena's last node). The `Language` has an eval
/// scheme `e.probe` for the root and `e.probe.<i>` for every node that
/// carries its own scheme; `case.roles` become `AstShapeDef` child roles
/// on the `probe` kind (ADR-0081).
pub fn build_probe(case: &Case) -> (Language, Ast) {
    let mut lang = Language::new();
    let mut ast = Ast::new();
    let mut child_ids: Vec<Value> = Vec::new();
    for (i, n) in case.nodes.iter().enumerate() {
        ast.push(AstNode::new(n.kind).with_payload(n.payload.clone()));
        child_ids.push(json!(i));
        if let Some(s) = &n.scheme {
            lang.eval
                .merge(EvalSchemeDef::new(
                    format!("e.probe.{i}"),
                    n.kind,
                    s.to_term().expect("probe node scheme must convert to Term"),
                ))
                .expect("merge probe node scheme");
        }
        if !n.roles.is_empty() || n.pattern_role.is_some() || !n.literal_children.is_empty() {
            let mut shape = AstShapeDef::new(format!("ast.probe.{i}"), n.kind);
            if !n.roles.is_empty() {
                shape = shape.with_children(
                    n.roles
                        .iter()
                        .map(|(r, optional)| ChildRoleSpec {
                            role: (*r).to_string(),
                            variadic: n.variadic_roles.contains(r),
                            optional: *optional,
                        })
                        .collect(),
                );
            }
            if let Some(pr) = n.pattern_role {
                shape = shape.with_pattern_role(pr);
            }
            for (role, kinds) in &n.literal_children {
                shape.literal_children.insert(
                    (*role).to_string(),
                    kinds.iter().map(|k| (*k).to_string()).collect(),
                );
            }
            lang.ast.merge(shape).expect("merge probe node shape");
        }
    }
    let mut root_payload = json!({ "children": child_ids });
    if let Some(extra) = &case.root_payload {
        if let (Some(root_obj), Some(extra_obj)) = (root_payload.as_object_mut(), extra.as_object())
        {
            for (k, v) in extra_obj {
                root_obj.insert(k.clone(), v.clone());
            }
        }
    }
    ast.push(AstNode::new("probe").with_payload(root_payload));
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.probe",
            "probe",
            tested_term(case)
                .to_term()
                .expect("tested term must convert to Term"),
        ))
        .expect("merge probe scheme");
    if !case.roles.is_empty() {
        lang.ast
            .merge(
                AstShapeDef::new("ast.probe", "probe").with_children(
                    case.roles
                        .iter()
                        .map(|r| ChildRoleSpec {
                            role: (*r).to_string(),
                            variadic: false,
                            optional: false,
                        })
                        .collect(),
                ),
            )
            .expect("merge probe roles");
    }
    (lang, ast)
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Run one `EvalEngine` under `catch_unwind` — a panic is an `Outcome`,
/// not a test abort.
fn run_engine(engine: &dyn EvalEngine, lang: &Language, ast: &Ast) -> Outcome {
    let ast = ast.clone();
    match catch_unwind(AssertUnwindSafe(|| {
        engine.eval(lang, &EvalRequest::new(ast))
    })) {
        Ok(Ok(v)) => Outcome::Val(v),
        Ok(Err(e)) => Outcome::Err(e.to_string()),
        Err(p) => Outcome::Panic(panic_message(p)),
    }
}

/// Run the pure term interpreter under `catch_unwind`.
fn run_term_interp(case: &Case) -> Outcome {
    let term = tested_term(case)
        .to_term()
        .expect("tested term must convert to Term");
    // The `Seq{0, Arr(input), _}` wrapper supplies the input; the
    // interpreter's own `input` parameter is unused context (Null).
    match catch_unwind(AssertUnwindSafe(|| {
        plugin_engine::term_interpreter::interpret(&term, Value::Null)
    })) {
        Ok(Ok(v)) => Outcome::Val(v),
        Ok(Err(e)) => Outcome::Err(e),
        Err(p) => Outcome::Panic(panic_message(p)),
    }
}

/// Run `case` on all four interpreters, in [`INTERPRETERS`] order.
pub fn run_case(case: &Case) -> [Outcome; 4] {
    let (lang, ast) = build_probe(case);
    [
        run_engine(&TreeWalkEngine::new(), &lang, &ast),
        run_engine(&BytecodeEngine::new(), &lang, &ast),
        run_engine(&CraneliftEngine::new(), &lang, &ast),
        run_term_interp(case),
    ]
}

fn panic_message(p: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic>".to_string()
    }
}

// ---------------------------------------------------------------------------
// Checking & reporting
// ---------------------------------------------------------------------------

/// Check one interpreter's outcome against its expectation.
pub fn check(expect: &Expect, outcome: &Outcome) -> Cell {
    let status = match (expect, outcome) {
        (Expect::Val(want), Outcome::Val(got)) => {
            if want == got {
                Status::Pass
            } else {
                Status::Fail
            }
        }
        (Expect::Err(sub), Outcome::Err(e)) => {
            if e.contains(sub) {
                Status::Pass
            } else {
                Status::Fail
            }
        }
        (Expect::DivVal(want), Outcome::Val(got)) => {
            if want == got {
                Status::Divergent
            } else {
                Status::Fail
            }
        }
        (Expect::DivErr(sub), Outcome::Err(e)) => {
            if e.contains(sub) {
                Status::Divergent
            } else {
                Status::Fail
            }
        }
        (Expect::Unsupported, Outcome::Err(_)) => Status::Unsupported,
        _ => Status::Fail,
    };
    Cell {
        status,
        outcome: outcome.clone(),
    }
}

/// Run the whole corpus, returning report rows.
pub fn run_suite(cases: &[Case]) -> Vec<Row> {
    cases
        .iter()
        .map(|case| Row {
            name: case.name,
            category: case.category,
            note: case.note,
            cells: case
                .expect
                .iter()
                .zip(run_case(case).iter())
                .map(|(e, o)| check(e, o))
                .collect(),
        })
        .collect()
}

fn status_tag(s: Status) -> &'static str {
    match s {
        Status::Pass => "PASS",
        Status::Divergent => "DIVERGENT",
        Status::Unsupported => "UNSUPPORTED",
        Status::Fail => "FAIL",
    }
}

/// Render the per-case × per-interpreter matrix plus the outcome detail
/// for every non-PASS cell.
pub fn format_report(rows: &[Row]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<34}{:<14}{:<14}{:<14}{:<14}\n",
        "case", INTERPRETERS[0], INTERPRETERS[1], INTERPRETERS[2], INTERPRETERS[3]
    ));
    out.push_str(&"-".repeat(90));
    out.push('\n');
    let mut cat = "";
    let mut counts = [0usize; 4]; // pass, divergent, unsupported, fail
    for row in rows {
        if row.category != cat {
            cat = row.category;
            out.push_str(&format!(
                "-- {cat} {}\n",
                "-".repeat(90 - cat.len().min(80))
            ));
        }
        let cells: Vec<String> = row
            .cells
            .iter()
            .map(|c| status_tag(c.status).to_string())
            .collect();
        out.push_str(&format!(
            "{:<34}{:<14}{:<14}{:<14}{:<14}\n",
            row.name, cells[0], cells[1], cells[2], cells[3]
        ));
        if !row.note.is_empty() {
            out.push_str(&format!("    note: {}\n", row.note));
        }
        for (i, cell) in row.cells.iter().enumerate() {
            counts[match cell.status {
                Status::Pass => 0,
                Status::Divergent => 1,
                Status::Unsupported => 2,
                Status::Fail => 3,
            }] += 1;
            if cell.status != Status::Pass {
                out.push_str(&format!(
                    "    {:<12} → {}\n",
                    INTERPRETERS[i],
                    cell.outcome.render()
                ));
            }
        }
    }
    out.push_str(&"-".repeat(90));
    out.push('\n');
    out.push_str(&format!(
        "totals: {} PASS, {} DIVERGENT, {} UNSUPPORTED, {} FAIL ({} cells)\n",
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        rows.len() * INTERPRETERS.len()
    ));
    out
}

/// Collect every `FAIL` cell as a human-readable line.
pub fn failures(rows: &[Row]) -> Vec<String> {
    let mut out = Vec::new();
    for row in rows {
        for (i, cell) in row.cells.iter().enumerate() {
            if cell.status == Status::Fail {
                out.push(format!(
                    "{} [{}]: {} → {}",
                    row.name,
                    INTERPRETERS[i],
                    status_tag(cell.status),
                    cell.outcome.render()
                ));
            }
        }
    }
    out
}
