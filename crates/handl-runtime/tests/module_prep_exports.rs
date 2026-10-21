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
use hnl_engine_exec::ImportResolver;
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse_pratt::PrattEngine;
use handl_runtime::{ModuleLoader, Runtime};
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
    let value = run(&root, src, r#"do { from "mod.hnl" import cell; cell[0] != null }"#)
        .expect("nested graded arrow evaluates");
    assert_eq!(value, json!(true));
    std::fs::remove_dir_all(&root).ok();
}

/// The inverse of the regression: an `export` the transform cannot make
/// sense of is a loud error, never a silent `{}`.
#[test]
fn separatorless_export_is_a_loud_error() {
    let root = scratch_dir("no-sep");
    let src = "export just_a_name\n";
    let err = run(&root, src, "1").expect_err("separator-less export must error");
    assert!(
        err.contains("export") && err.contains("cannot transform"),
        "error should name the export declaration, got: {err}"
    );
    std::fs::remove_dir_all(&root).ok();
}
