//! Regression tests for `module_prep`'s export-to-object rewrite.
//!
//! The transform used to recognize a top-level `export` declaration only
//! when the `export ` prefix sat at column 0 of a *line* — so an export
//! written single-line after another statement, indented, or sharing a
//! line with a sibling silently vanished: the module evaluated to `{}`
//! with no error. The rewrite is now structure-aware (statements are
//! segmented by bracket depth, strings, and comments — not by lines)
//! and a declaration it cannot transform raises `LangError` instead of
//! being dropped.
//!
//! The canonical reproduction is the `math-handl` helpers — `clamp`,
//! `sign`, `is_finite` — as one-line `export x: T := (…) ~> do { if …
//! { … } else { … } }` declarations.

use std::path::PathBuf;
use std::rc::Rc;

use handl_language::Source;
use handl_runtime::{ModuleLoader, Runtime};
use hnl_exec::ImportResolver;
use hnl_exec_tree_walk::TreeWalkEngine;
use hnl_tool_lex_pattern::DataLexerEngine;
use hnl_tool_parse_pratt::PrattEngine;
use serde_json::json;

/// A unique temp directory for this test process's module fixtures.
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hnl-prep-export-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Build a `ModuleLoader` + `Runtime` rooted at `module_root` — the same
/// wiring `dependency_import.rs` uses.
fn loader_and_runtime(module_root: PathBuf) -> (Rc<ModuleLoader>, Runtime) {
    let kit = handl_kit_data_prelude::assemble().expect("canonical kit assembles");
    let lang_rc = Rc::new(kit.lang().clone());

    let loader = Rc::new(ModuleLoader::new(
        Rc::clone(&lang_rc),
        Box::new(DataLexerEngine::new()),
        Box::new(PrattEngine::new()),
        Some(module_root),
    ));
    let mut loader_eval =
        TreeWalkEngine::new().with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>);
    for (name, val) in &kit.lang().globals {
        loader_eval = loader_eval.with_global(name, val.clone());
    }
    loader.set_eval_engine(Box::new(loader_eval));

    let runtime = Runtime::new(kit.lang().clone())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(
            TreeWalkEngine::new()
                .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>),
        ))
        .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>);
    (loader, runtime)
}

/// Write `src` to `<dir>/mod.hnl`, load the module record, and evaluate
/// `expr` (which may `from "mod.hnl" import …` and apply the exports).
fn run(dir: &PathBuf, src: &str, expr: &str) -> Result<serde_json::Value, String> {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("mod.hnl"), src).unwrap();
    let (loader, rt) = loader_and_runtime(dir.clone());
    loader
        .resolve_import("mod.hnl")
        .map_err(|e| format!("resolve_import: {e}"))?;
    rt.run(&Source::new(expr)).map_err(|e| format!("eval: {e}"))
}

const CLAMP_ONE_LINE: &str = "export clamp: Clamp := (x, lo, hi) ~> do { if x < lo { lo } else { if x > hi { hi } else { x } } }\n";
const SIGN_ONE_LINE: &str =
    "export sign: Sign := (x) ~> do { if x > 0 { 1.0 } else { if x < 0 { -1.0 } else { 0.0 } } }\n";
const IS_FINITE_ONE_LINE: &str = "export is_finite: IsFinite := (x) ~> do { if x != x { false } else { if x == 0 { true } else { x != x + x } } }\n";

/// The reported bug: single-line `export x: T := (…) ~> do { if … { … }
/// else { if … { … } else { … } } }` exports must bind and evaluate
/// exactly like their multi-line equivalents.
#[test]
fn single_line_do_block_exports_evaluate() {
    let root = scratch_dir("one-line");

    let value = run(
        &root.join("clamp"),
        CLAMP_ONE_LINE,
        r#"do { from "mod.hnl" import clamp; clamp(5, 1, 3) }"#,
    )
    .expect("single-line clamp export must evaluate");
    assert_eq!(value, json!(3), "clamp(5, 1, 3)");

    let value = run(
        &root.join("sign"),
        SIGN_ONE_LINE,
        r#"do { from "mod.hnl" import sign; sign(-2) }"#,
    )
    .expect("single-line sign export must evaluate");
    assert_eq!(value, json!(-1.0), "sign(-2)");

    let value = run(
        &root.join("is_finite"),
        IS_FINITE_ONE_LINE,
        r#"do { from "mod.hnl" import is_finite; is_finite(5) }"#,
    )
    .expect("single-line is_finite export must evaluate");
    assert_eq!(value, json!(true), "is_finite(5)");

    std::fs::remove_dir_all(&root).ok();
}

/// A multi-line `export` body keeps working (the newline-inside-braces
/// case the old transform relied on).
#[test]
fn multi_line_do_block_export_still_evaluates() {
    let root = scratch_dir("multi-line");
    let src = "export clamp: Clamp := (x, lo, hi) ~> do {\n  if x < lo {\n    lo\n  } else {\n    if x > hi {\n      hi\n    } else {\n      x\n    }\n  }\n}\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import clamp; clamp(5, 1, 3) }"#,
    )
    .expect("multi-line clamp export must evaluate");
    assert_eq!(value, json!(3));
    std::fs::remove_dir_all(&root).ok();
}

/// An `export` sharing its line with other statements is still an
/// export: the import tail and the declaration split at the statement
/// boundary, not the newline.
#[test]
fn export_after_another_statement_on_one_line() {
    let root = scratch_dir("shared-line");
    // The whole module on a single line — imports and export share it.
    let src = "from \"type\" import { Number } export sign: Sign := (x) ~> do { if x > 0 { 1.0 } else { -1.0 } }\n";
    let value = run(&root, src, r#"do { from "mod.hnl" import sign; sign(-2) }"#)
        .expect("one-line module must export `sign`");
    assert_eq!(value, json!(-1.0));
    std::fs::remove_dir_all(&root).ok();
}

/// Two exports on one line (`;`-separated or adjacency): both bind.
#[test]
fn multiple_exports_on_one_line() {
    let root = scratch_dir("two-exports");
    let src = "export inc := (x) ~> x + 1; export dec := (x) ~> x - 1\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import inc; from "mod.hnl" import dec; inc(dec(5)) }"#,
    )
    .expect("both single-line exports must bind");
    assert_eq!(value, json!(5), "inc(dec(5)) = inc(4) = 5");
    std::fs::remove_dir_all(&root).ok();
}

/// An indented `export` is a top-level declaration, not an expression
/// tail — it must not be swallowed into the preceding statement (or
/// into nothing, yielding the old silent `{}`).
#[test]
fn indented_export_is_still_an_export() {
    let root = scratch_dir("indented");
    let src = "  export inc := (x) ~> x + 1\n";
    let value = run(&root, src, r#"do { from "mod.hnl" import inc; inc(4) }"#)
        .expect("indented export must still bind `inc`");
    assert_eq!(value, json!(5));
    std::fs::remove_dir_all(&root).ok();
}

/// Agda-style erasure: `export Name := (Params) ~[N]~> Codomain` — a
/// signature export whose RHS is a graded-arrow signature — must NOT
/// evaluate its boundary children. `Result<Output, Spawn>` is a type-level
/// generic application (a `dimension` node): evaluating it applies the
/// former as a runtime arrow and crashes. The export binds `null` — the
/// same value type-only `export Name: Type` gets — and importing it is a
/// working export.
#[test]
fn graded_signature_export_does_not_evaluate_type_children() {
    let root = scratch_dir("sig-export");
    // `Pair` is a parameter'd export (a former); `Sig`'s codomain is the
    // un-evaluable generic application `Pair<Output, Output>` — exactly
    // the `cli` port's `Run := ([String]) ~[1]~> Result<Output, Spawn>`
    // shape. Before the fix the import crashed eval with
    // "callee is not an arrow-value".
    let src = "export Output := { code: Int, msg: String }\n\
               export Sig := (Int) ~[1]~> Pair<Output, Output>\n\
               export f := (x) ~> x + 1\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import { Output, Sig, f }; { sig: Sig, sum: f(41) } }"#,
    )
    .expect("signature export must not evaluate its type children");
    assert_eq!(value["sig"], json!(null), "signature export is erased");
    assert_eq!(value["sum"], json!(42), "sibling value export unaffected");
    std::fs::remove_dir_all(&root).ok();
}

/// The inverse erasure boundary: an export RHS that is a genuine n-cell
/// *value* — `f ~[2]~> g`, `"a >>> b" ~[2]~> "id"` — must bind the Arrow
/// cell it evaluates to, not `null`. Before `graded_arrow_is_signature`
/// the depth-0 `~[` erased these wholesale (the footgun). Grade-2+ and
/// value-led LHS operands (`f`, `"…"`) are cells; a grade-1 lowercase
/// term `f ~[1]~> g` is a cell too.
#[test]
fn n_cell_value_exports_are_preserved() {
    let root = scratch_dir("cell-export");
    let src = "f := (x) ~> x\ng := (x) ~> x + 1\n\
               export alpha := f ~[2]~> g\n\
               export rule := \"a >>> b\" ~[2]~> \"id\"\n\
               export beta := f ~[1]~> g\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import { alpha, rule, beta }; { a: alpha != null, l: rule != null, b: beta != null } }"#,
    )
    .expect("cell exports must bind");
    assert_eq!(value["a"], json!(true), "f ~[2]~> g is a value — kept");
    assert_eq!(value["l"], json!(true), "string-law cell is a value — kept");
    assert_eq!(
        value["b"],
        json!(true),
        "lowercase-led grade-1 term is a value — kept"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// A paren-less `Type ~[1]~> Type` RHS is still a signature — the domain
/// is a `Capitalized` type head, so it erases like the `(Params)` shape.
/// And a lambda whose *body* holds a depth-0 cell
/// (`export sigma := (Q, f, Sf) ~> Q ~[2]~> (f >>> Sf(Q))`, the
/// `lib-type` dependent shape) still binds the closure.
#[test]
fn bare_type_signature_erases_lambda_cell_body_keeps() {
    let root = scratch_dir("bare-sig");
    let src = "export BareSig := Int ~[1]~> Result<Int, String>\n\
               export sigma := (Q, f, Sf) ~> Q ~[2]~> (f >>> Sf(Q))\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import { BareSig, sigma }; { sig: BareSig, s: sigma != null } }"#,
    )
    .expect("bare-type signature erases; cell-bodied lambda binds");
    assert_eq!(
        value["sig"],
        json!(null),
        "Int ~[1]~> Result<…> is a signature"
    );
    assert_eq!(
        value["s"],
        json!(true),
        "lambda with a cell body is a value"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// `export f: A ~[N]~> B := body` — an *annotated* implementation — still
/// binds `f` to the body: the annotation between `:` and `:=` is erased,
/// the arrow body evaluates normally.
#[test]
fn annotated_export_keeps_body_evaluation() {
    let root = scratch_dir("annotated-export");
    let src = "export f: (Int) ~[1]~> Pair<Int, Int> := (x) ~> x * 2\n";
    let value = run(&root, src, r#"do { from "mod.hnl" import f; f(21) }"#)
        .expect("annotated export body evaluates");
    assert_eq!(value, json!(42));
    std::fs::remove_dir_all(&root).ok();
}

/// A `~[n]~>` nested inside a larger value expression is runtime cell
/// data, not a signature: only a graded arrow at depth 0 of the RHS is
/// erased, so a list containing a `dim_arrow` cell still evaluates.
#[test]
fn nested_graded_arrow_still_evaluates() {
    let root = scratch_dir("nested-graded");
    let src = "export cell := [ (x) ~> x ~[2]~> (y) ~> y ]\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import cell; cell[0] != null }"#,
    )
    .expect("nested graded arrow evaluates");
    assert_eq!(value, json!(true));
    std::fs::remove_dir_all(&root).ok();
}

/// The inverse of the regression: an `export` the transform cannot make
/// sense of is a loud error, never a silent `{}`.
#[test]
fn separatorless_export_is_a_loud_error() {
    let root = scratch_dir("no-sep");
    // `(paren)` is separator-less and not a valid identifier list — a
    // bare `export name` is a re-export declaration now (see the
    // `bare_export_*` tests), so this is still untransformable.
    let src = "export (paren)\n";
    let err = run(&root, src, "1").expect_err("separator-less export must error");
    assert!(
        err.contains("export") && err.contains("cannot transform"),
        "error should name the export declaration, got: {err}"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// `export data F := { C : R, … }` — the canonical family-record
/// declaration under `export` — must behave like the `data` declaration
/// plus exports of the family *and* each constructor. The declaration is
/// preserved verbatim inside the prepared `do` block (the desugar pass
/// expands it into the former/ctor/recursor assigns there) and the
/// record projects `F`, `Zero`, `Succ`, … — previously the `export`
/// bound a bogus `data F` name and the family residue failed.
#[test]
fn export_data_record_exports_family_and_ctors() {
    let root = scratch_dir("export-data-record");
    let src = "export data Nat := { Zero : Nat, Succ : (pred : Nat) ~> Nat }\nexport two := Succ(Succ(Zero))\n";

    // Name seeding sees every bound name.
    let names = handl_runtime::module_prep::module_export_names(src);
    for name in ["Nat", "Zero", "Succ", "two"] {
        assert!(
            names.iter().any(|n| n == name),
            "missing export {name}: {names:?}"
        );
    }

    // The ctor values cross the module boundary as data — `match`
    // dispatches on them importer-side via the `C()`/`C(x)` call-pattern
    // forms (bare `Zero:` is a binder pattern, and *applying* a ctor
    // arrow cross-module is a separate pre-existing limitation, so the
    // interesting value is minted inside the module).
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import two, Zero, Succ; match two { Zero(): 0, Succ(p): match p { Zero(): 1, Succ(q): 2 } } }"#,
    )
    .expect("export data module must evaluate");
    assert_eq!(value, json!(2), "two := Succ(Succ(Zero))");
    std::fs::remove_dir_all(&root).ok();
}

/// `export data F := C0 | C1(…)` — the `:=` constructor-arm
/// spelling (`:=` is the only binding operator; the old `::` is gone)
/// must carry the decl verbatim and export the family plus each
/// constructor arm's name.
#[test]
fn export_data_colon_form_exports_family_and_ctors() {
    let root = scratch_dir("export-data-colon");
    let src = "export data Nat := Zero | Succ(n : Nat)\nexport one := Succ(Zero)\n";

    let names = handl_runtime::module_prep::module_export_names(src);
    for name in ["Nat", "Zero", "Succ", "one"] {
        assert!(
            names.iter().any(|n| n == name),
            "missing export {name}: {names:?}"
        );
    }

    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import one, Zero, Succ; match one { Zero(): 0, Succ(p): match p { Zero(): 1, Succ(q): 2 } } }"#,
    )
    .expect("export data := module must evaluate");
    assert_eq!(value, json!(1), "one := Succ(Zero)");
    std::fs::remove_dir_all(&root).ok();
}

/// `export data` mixed with ordinary exports keeps source order: the
/// decl's assigns bind before the sibling binding that references them.
#[test]
fn export_data_alongside_plain_exports() {
    let root = scratch_dir("export-data-mixed");
    let src = "export double : Number ~> Number := n ~> n * 2\nexport data Wrap := { Mk : (x : Number) ~> Wrap, Empty : Wrap }\nexport wrapped := Mk(21)\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import wrapped, Mk, Empty; match wrapped { Mk(x): x, _: 0 } }"#,
    )
    .expect("mixed export data module must evaluate");
    // `wrapped` is `Mk(21)` — matching extracts the payload.
    assert_eq!(value, json!(21));
    // `double` still exports and evaluates.
    let value = run(
        &root,
        "export double : Number ~> Number := n ~> n * 2\n",
        r#"do { from "mod.hnl" import double; double(21) }"#,
    )
    .expect("plain export still works");
    assert_eq!(value, json!(42));
    std::fs::remove_dir_all(&root).ok();
}

/// Regression: a no-`export` module whose last top-level statement is a
/// bare `do { … }`/record literal must evaluate to *that* record — not
/// to a `{ name: name }` projection of the preceding bindings. Any
/// top-level binding before the trailing record (a private helper, or
/// the `local := "spec"` line a host-module `from "spec" import * as m`
/// lowers to) used to absorb the record segment into the binding and
/// export only the binding names — lib-pred's `from "char" import * as
/// char` + trailing combinator record evaluated to
/// `{ "char": "char" }`, so every `pred.*` call failed
/// `callee is not an arrow-value`.
///
/// I-007: named imports still project the full module record, so the
/// trailing-record fields stay reachable by name; star imports bind
/// only *declared* exports (empty here — see
/// `star_import_of_exportless_module_binds_empty_namespace`).
#[test]
fn no_export_trailing_do_record_is_module_value() {
    let root = scratch_dir("tail-do-record");
    let src = "helper := x ~> x + 1\ndo {\n  { f: helper, g: x ~> x * 2 }\n}\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import { f, g }; { a: f(20), b: g(21) } }"#,
    )
    .expect("trailing do-record module must evaluate");
    assert_eq!(value, json!({ "a": 21, "b": 42 }));
    std::fs::remove_dir_all(&root).ok();
}

/// Same shape with a bare record literal tail (no `do` wrapper) and a
/// host-module wildcard import ahead of it — the lib-pred reproduction:
/// `from "nohost" import * as nh` lowers to `nh := "nohost"`, a binding
/// that must not swallow the trailing record.
#[test]
fn host_import_then_trailing_record_keeps_record() {
    let root = scratch_dir("host-import-tail");
    let src = "from \"nohost\" import * as nh\nhelper := x ~> x + 1\n{ f: helper }\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import f; f(41) }"#,
    )
    .expect("module must evaluate to its trailing record");
    assert_eq!(value, json!(42));
    std::fs::remove_dir_all(&root).ok();
}

/// Wrapped-RHS continuations still join the binding: an operator-led
/// operand on the next line and an `if` whose `{ … }` block opens on
/// the following line are part of the expression, not new statements —
/// the trailing `do { … }` record remains the module value.
#[test]
fn wrapped_rhs_continuations_still_join() {
    let root = scratch_dir("wrapped-rhs");
    let src = "x := 1 +\n2\ny := if true\n{ 10 } else { 20 }\ndo {\n  { a: x, b: y }\n}\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import { a, b }; a + b }"#,
    )
    .expect("wrapped RHS module must evaluate");
    assert_eq!(value, json!(13), "x = 1 + 2 = 3, y = 10");
    std::fs::remove_dir_all(&root).ok();
}

/// I-007: star forms (`import * as m`, `import "…"`) bind only the
/// module's *declared* exports. A module with no `export` declarations
/// yields an empty namespace even though its evaluated value is the
/// trailing record — `m.f` is absent while `from "…" import f` still
/// reaches the record field.
#[test]
fn star_import_of_exportless_module_binds_empty_namespace() {
    let root = scratch_dir("star-empty-ns");
    let src = "helper := x ~> x + 1\ndo {\n  { f: helper }\n}\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import * as m; m }"#,
    )
    .expect("star import of export-less module must evaluate");
    assert_eq!(value, json!({}));
    // Named import still sees the trailing record's fields.
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import f; f(41) }"#,
    )
    .expect("named import still projects the module record");
    assert_eq!(value, json!(42));
    std::fs::remove_dir_all(&root).ok();
}

/// Coverage provenance end-to-end: module prep re-serializes the source,
/// but the registered `CoverageSource` carries a prepared-line → file-line
/// map, so hits and declaration lines resolve to the `.hnl` file on disk
/// — comments and blank lines included in the fixture must not shift them.
#[test]
fn coverage_reports_file_lines() {
    let root = scratch_dir("coverage");
    // File lines: 1 `--` comment, 2 blank, 3 `export base`, 4 blank,
    // 5 `--` comment, 6 `export answer`.
    let src = "-- module under test\n\nexport base := 40\n\n-- answer lives on line 6\nexport answer := base + 2\n";
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("mod.hnl"), src).unwrap();
    let (loader, rt) = loader_and_runtime(root.clone());
    // Install before `resolve_import` — resolution may eagerly load (and
    // evaluate) the module, and coverage only registers while a session
    // is active on this thread.
    let session = hnl_exec::coverage::CoverageSession::new();
    session.install_local();
    loader
        .resolve_import("mod.hnl")
        .map_err(|e| format!("resolve_import: {e}"))
        .unwrap();
    let value = rt
        .run(&Source::new(
            r#"do { from "mod.hnl" import answer; answer }"#,
        ))
        .map_err(|e| format!("eval: {e}"))
        .expect("module must evaluate");
    assert_eq!(value, json!(42));
    let report = session.report();
    hnl_exec::coverage::CoverageSession::uninstall_local();

    let s = report
        .sources
        .iter()
        .find(|s| s.origin.as_deref().is_some_and(|o| o.ends_with("mod.hnl")))
        .unwrap_or_else(|| {
            panic!(
                "module coverage source registered; sources: {:?}",
                report
                    .sources
                    .iter()
                    .map(|s| (s.name.clone(), s.origin.clone(), s.file_line_hits.is_some()))
                    .collect::<Vec<_>>()
            )
        });
    assert!(s.line_map.is_some(), "{s:?}");
    let hits = s.file_line_hits.as_ref().expect("file-line hits");
    // Hits resolve to file lines 3 (`base := 40`) and 6 (`answer := …`)
    // only — never the comment/blank/generated lines.
    let hit_lines: Vec<u32> = hits.iter().map(|(l, _)| *l).collect();
    assert!(hit_lines.contains(&3), "{hit_lines:?}");
    assert!(hit_lines.contains(&6), "{hit_lines:?}");
    assert!(
        hit_lines.iter().all(|l| [3, 6].contains(l)),
        "{hit_lines:?}"
    );
    for name in ["base", "answer"] {
        let d = s
            .decls
            .iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("decl {name} in {s:?}"));
        let want = if name == "base" { 3 } else { 6 };
        assert_eq!(d.file_line, Some(want), "{name}: {d:?}");
    }
    std::fs::remove_dir_all(&root).ok();
}

/// Bare `export <name>[, <name>…]` re-exports a private `:=` binding:
/// the name joins the export record while the binding stays a do-block
/// assignment, so both named and star imports reach it.
#[test]
fn bare_export_reexports_private_binding() {
    let root = scratch_dir("reexport");
    let src = "inc := x ~> x + 1\ndouble := x ~> x * 2\nexport inc, double\n";
    let value = run(
        &root.join("named"),
        src,
        r#"do { from "mod.hnl" import double; double(21) }"#,
    )
    .expect("named import of re-exported binding must evaluate");
    assert_eq!(value, json!(42));

    let value = run(
        &root.join("star"),
        src,
        r#"do { from "mod.hnl" import * as m; m.inc(1) }"#,
    )
    .expect("star import must see the re-exported name");
    assert_eq!(value, json!(2));
    std::fs::remove_dir_all(&root).ok();
}

/// A re-export may precede its binding — resolution happens after the
/// whole module is scanned.
#[test]
fn bare_export_before_its_binding() {
    let root = scratch_dir("reexport-fwd");
    let src = "export later\nlater := x ~> x * 3\n";
    let value = run(
        &root,
        src,
        r#"do { from "mod.hnl" import later; later(14) }"#,
    )
    .expect("forward re-export must evaluate");
    assert_eq!(value, json!(42));
    std::fs::remove_dir_all(&root).ok();
}

/// Re-exporting an already-exported name is a no-op, not a duplicate
/// record field.
#[test]
fn bare_export_of_exported_name_is_noop() {
    let root = scratch_dir("reexport-noop");
    let src = "export f := x ~> x + 1\nexport f\n";
    let value = run(&root, src, r#"do { from "mod.hnl" import f; f(41) }"#)
        .expect("duplicate re-export must evaluate");
    assert_eq!(value, json!(42));
    std::fs::remove_dir_all(&root).ok();
}

/// A bare re-export naming nothing is a loud prep error — never a
/// silent `{}` module.
#[test]
fn bare_export_unknown_name_is_loud() {
    let root = scratch_dir("reexport-unknown");
    let err =
        run(&root, "export nope\n", r#"1"#).expect_err("re-export of an unknown name must fail");
    assert!(
        err.contains("does not name a top-level binding"),
        "unexpected error: {err}"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// A bare re-export can name a local bound by a hoisted import — the
/// import binds it at eval time, so `name: name` reaches it.
#[test]
fn bare_export_of_import_bound_local() {
    let root = scratch_dir("reexport-import");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("dep.hnl"), "export g := x ~> x + 10\n").unwrap();
    let src = "from \"dep.hnl\" import g\nexport g\n";
    let value = run(&root, src, r#"do { from "mod.hnl" import g; g(32) }"#)
        .expect("re-export of an import-bound local must evaluate");
    assert_eq!(value, json!(42));
    std::fs::remove_dir_all(&root).ok();
}
