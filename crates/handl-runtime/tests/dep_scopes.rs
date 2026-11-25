//! `pkg.hnl` dep scopes (P-013 T-150): a dependency whose `deps` entry
//! declares `modules:`/`atoms:` exposes only that surface to the
//! consuming package — enforced in [`ModuleLoader`] at import time, so
//! eval sees exactly the declared atoms.
//!
//! What is exercised (through [`ModuleLoader::with_dep_scopes`]):
//!
//! - `atoms: ["mri.parse"]` keeps `parse` and drops `hidden` from the
//!   imported record — `m.hidden` is absent at eval;
//! - `modules: [...]` gates whole dep modules — an excluded module's
//!   import is an error, not an empty record;
//! - unscoped deps are untouched (the legacy whole-surface behavior).

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use handl_language::Source;
use handl_platform::PkgDepScope;
use handl_runtime::{ModuleLoader, Runtime};
use handl_value::TaggedValue;
use hnl_exec::ImportResolver;
use hnl_exec_tree_walk::TreeWalkEngine;
use hnl_tool_lex_pattern::DataLexerEngine;
use hnl_tool_parse_pratt::PrattEngine;
use serde_json::json;

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hnl-dep-scope-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn loader_and_runtime(
    module_root: PathBuf,
    deps: HashMap<String, PathBuf>,
    scopes: HashMap<String, PkgDepScope>,
) -> (Rc<ModuleLoader>, Runtime) {
    let kit = handl_kit_data_prelude::assemble().expect("canonical kit assembles");
    let lang_rc = Rc::new(kit.lang().clone());

    let loader = Rc::new(
        ModuleLoader::new(
            Rc::clone(&lang_rc),
            Box::new(DataLexerEngine::new()),
            Box::new(PrattEngine::new()),
            Some(module_root),
        )
        .with_dependencies(deps)
        .with_dep_scopes(scopes),
    );
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

fn atoms_scope(fields: &[&str]) -> PkgDepScope {
    PkgDepScope {
        modules: vec![],
        atoms: fields.iter().map(|f| format!("mri.{f}")).collect(),
        caps: vec![],
    }
}

/// Fixture: dep `mri` (bare-name → `<dep>/mri.hnl`) exporting `parse` and
/// `hidden`; consumer project dir empty (imports go through the deps map).
fn fixture(tag: &str) -> (PathBuf, PathBuf, HashMap<String, PathBuf>) {
    let root = scratch_dir(tag);
    let project = root.join("project");
    let dep_mri = root.join("vendor/mri");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&dep_mri).unwrap();
    std::fs::write(
        dep_mri.join("mri.hnl"),
        "export parse := (x) ~> x + 1\nexport hidden := (x) ~> x * 10\n",
    )
    .unwrap();
    let deps = HashMap::from([("mri".to_string(), dep_mri)]);
    (root, project, deps)
}

/// `atoms:` keeps the declared export and drops the rest from the record
/// the import binds — at both the tagged-record and eval levels.
#[test]
fn atom_scope_filters_the_imported_record() {
    let (root, project, deps) = fixture("atom");
    let scopes = HashMap::from([("mri".to_string(), atoms_scope(&["parse"]))]);
    let (loader, rt) = loader_and_runtime(project, deps, scopes);

    // The tagged record the `import` op hands out has `parse`, not `hidden`.
    let tagged = loader
        .resolve_import_tagged("mri")
        .expect("scoped dep must still resolve");
    let TaggedValue::Obj(map) = tagged.as_ref() else {
        panic!("expected an export record, got {tagged:?}");
    };
    assert!(map.contains_key("parse"), "declared atom missing: {map:?}");
    assert!(
        !map.contains_key("hidden"),
        "scoped-out atom leaked: {map:?}"
    );

    // Eval end to end: the declared atom applies; the scoped-out one is
    // absent from the bound record.
    let value = rt
        .run(&Source::new(
            r#"do { from "mri" import * as m; m.parse(4) }"#,
        ))
        .expect("declared atom must evaluate");
    assert_eq!(value, json!(5));

    // The scoped-out field is absent from the bound record, so applying
    // it fails (the field access yields a non-arrow callee).
    rt.run(&Source::new(
        r#"do { from "mri" import * as m; m.hidden(4) }"#,
    ))
    .expect_err("scoped-out atom must fail at eval");

    std::fs::remove_dir_all(&root).ok();
}

/// `modules:` gates whole dep modules: an import naming an excluded
/// module is an error rather than a silently empty record.
#[test]
fn module_scope_excludes_other_modules() {
    let root = scratch_dir("modules");
    let project = root.join("project");
    let dep_utils = root.join("vendor/utils");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(dep_utils.join("math")).unwrap();
    std::fs::create_dir_all(dep_utils.join("str")).unwrap();
    std::fs::write(
        dep_utils.join("math/index.hnl"),
        "export abs := (x) ~> if x < 0 { 0 - x } else { x }\n",
    )
    .unwrap();
    std::fs::write(dep_utils.join("str/index.hnl"), "export len := (s) ~> 0\n").unwrap();

    let deps = HashMap::from([("utils".to_string(), dep_utils)]);
    let scopes = HashMap::from([(
        "utils".to_string(),
        PkgDepScope {
            modules: vec!["math".to_string()],
            atoms: vec![],
            caps: vec![],
        },
    )]);
    let (loader, _rt) = loader_and_runtime(project, deps, scopes);

    // `utils/math` is inside the scope — whole module surface.
    let tagged = loader
        .resolve_import_tagged("utils/math")
        .expect("declared module must resolve");
    let TaggedValue::Obj(map) = tagged.as_ref() else {
        panic!("expected an export record, got {tagged:?}");
    };
    assert!(map.contains_key("abs"));

    // `utils/str` is outside it — a hard error, not an empty binding.
    let err = loader
        .resolve_import_tagged("utils/str")
        .expect_err("excluded module must fail");
    assert!(
        err.to_string().contains("str") && err.to_string().contains("scope"),
        "unexpected error: {err}"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// Deps without a scope entry behave exactly as before — the whole
/// export record crosses the boundary.
#[test]
fn unscoped_deps_keep_the_whole_surface() {
    let (root, project, deps) = fixture("unscoped");
    let (_loader, rt) = loader_and_runtime(project, deps, HashMap::new());
    let value = rt
        .run(&Source::new(
            r#"do { from "mri" import * as m; m.hidden(4) }"#,
        ))
        .expect("unscoped dep exposes every export");
    assert_eq!(value, json!(40));
    std::fs::remove_dir_all(&root).ok();
}
