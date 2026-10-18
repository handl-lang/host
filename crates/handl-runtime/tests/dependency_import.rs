//! `.handl.toml` `[dependencies]` name resolution: a `.hnl` package imports
//! a dependency's files **by package name** — `from "dep/file" import …` or
//! `from "dep" import …` — instead of a `../` filesystem path. This is the
//! seam the ADR-0128 Phase-5 `.hnl` repo split (`c/`, `web/`, …) stands on:
//! a consuming repo declares the dep once in its manifest and the sources
//! carry no cross-repo relative paths.
//!
//! What is exercised (all through [`ModuleLoader::with_dependencies`]):
//!
//! - `from "pkg/inner" import f` — dep name + remainder resolves
//!   `<deps["pkg"]>/inner.hnl`;
//! - `from "pkg" import g` — a bare dep name resolves the package dir's
//!   `<name>.hnl` main file;
//! - `import "pkg/inner.hnl"` — the splice form resolves the same way;
//! - inside a dep package, its own `from "./x"`/`from "../x"` imports keep
//!   resolving relative to the dep file's directory (in-repo imports are
//!   untouched — only cross-repo edges go through the name table);
//! - a dep name shadows a same-named directory next to the importer
//!   (matching `ModuleLoader::resolve_path` precedence);
//! - the git-package→dir bridge: a `.handl.toml` that names a dep
//!   *portably* by `git` source plus a `[patch]` local override resolves
//!   the sibling checkout — `handl_platform::resolve_dependencies`
//!   produces the name→dir map the loader consumes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use handl_language::{Source, Value};
use handl_plugin_engine_exec::ImportResolver;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{ModuleLoader, Runtime};
use serde_json::json;

/// A unique temp directory for this test process's module fixtures.
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hnl-dep-import-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Build a `ModuleLoader` + `Runtime` rooted at `module_root` with `deps` as
/// the `[dependencies]` name → package-dir map.
fn loader_and_runtime(module_root: PathBuf, deps: HashMap<String, PathBuf>) -> (Rc<ModuleLoader>, Runtime) {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");
    let lang_rc = Rc::new(kit.lang().clone());

    let loader = Rc::new(
        ModuleLoader::new(
            Rc::clone(&lang_rc),
            Box::new(DataLexerEngine::new()),
            Box::new(PrattEngine::new()),
            Some(module_root),
        )
        .with_dependencies(deps),
    );
    let mut loader_eval = TreeWalkEngine::new()
        .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>);
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

/// `from "ws-c" import prepare` resolves `prepare` from the dep's main file
/// (`<deps["ws-c"]>/ws-c.hnl`), and `from "libc/socket" import socket`
/// resolves `<deps["libc"]>/socket.hnl` — both by name, no `../` in sight.
#[test]
fn dependency_name_specs_resolve_inside_the_dep_dir() {
    let root = scratch_dir("names");
    let project = root.join("project");
    let dep_ws = root.join("vendor/ws-c");
    let dep_libc = root.join("vendor/libc-c-linux-x86_64");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&dep_ws).unwrap();
    std::fs::create_dir_all(&dep_libc).unwrap();

    std::fs::write(dep_ws.join("ws-c.hnl"), "export prepare := (x) ~> x + 1\n").unwrap();
    std::fs::write(dep_libc.join("socket.hnl"), "export socket := (x) ~> x * 2\n").unwrap();
    std::fs::write(
        project.join("consumer.hnl"),
        concat!(
            "from \"ws-c\" import prepare\n",
            "from \"libc/socket\" import socket\n",
            "export go := (x) ~> socket(prepare(x))\n",
        ),
    )
    .unwrap();

    let deps = HashMap::from([
        ("ws-c".to_string(), dep_ws.clone()),
        ("libc".to_string(), dep_libc.clone()),
    ]);
    let (loader, rt) = loader_and_runtime(project.clone(), deps);

    // The module record loads — both dep-name imports resolved to files.
    let value = loader
        .resolve_import("consumer.hnl")
        .expect("dep-name imports must resolve through the deps table");
    let Value::Object(obj) = &value else {
        panic!("module record should be an object, got {value}");
    };
    assert!(obj.contains_key("go"), "module should export `go`: {value}");

    // And end to end: importing `go` and applying it reaches both dep files:
    // `socket(prepare(3))` = `(3 + 1) * 2` = 8.
    let value = rt
        .run(&Source::new(
            r#"do { from "consumer.hnl" import go; go(3) }"#,
        ))
        .expect("import + apply should reach both dep modules");
    assert_eq!(value, json!(8));

    std::fs::remove_dir_all(&root).ok();
}

/// Inside a dep package, `./`-relative imports still resolve relative to the
/// dep file's own directory — the name seam does not disturb in-repo paths.
#[test]
fn dep_package_internal_relative_imports_still_resolve() {
    let root = scratch_dir("nested");
    let project = root.join("project");
    let dep = root.join("vendor/libc-c-linux-x86_64");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&dep).unwrap();

    // `socket.hnl` pulls `wrap` from the sibling `./support.hnl` — relative,
    // resolved against the dep file's directory.
    std::fs::write(dep.join("support.hnl"), "export wrap := (x) ~> x + 10\n").unwrap();
    std::fs::write(
        dep.join("socket.hnl"),
        "from \"./support\" import wrap\nexport socket := (x) ~> wrap(x)\n",
    )
    .unwrap();
    std::fs::write(
        project.join("consumer.hnl"),
        "from \"libc/socket\" import socket\nexport go := (x) ~> socket(x)\n",
    )
    .unwrap();

    let deps = HashMap::from([("libc".to_string(), dep)]);
    let (_loader, rt) = loader_and_runtime(project.clone(), deps);

    let value = rt
        .run(&Source::new(
            r#"do { from "consumer.hnl" import go; go(5) }"#,
        ))
        .expect("dep file's own relative imports must keep resolving");
    assert_eq!(value, json!(15));

    std::fs::remove_dir_all(&root).ok();
}

/// A dep name shadows a same-named directory next to the importer: `libc`
/// maps to the vendored dir even though `project/libc/socket.hnl` also
/// exists (`ModuleLoader::resolve_path` precedence).
#[test]
fn dependency_names_shadow_same_named_relative_dirs() {
    let root = scratch_dir("shadow");
    let project = root.join("project");
    let local = project.join("libc");
    let vendored = root.join("vendor/libc-c-linux-x86_64");
    std::fs::create_dir_all(&local).unwrap();
    std::fs::create_dir_all(&vendored).unwrap();

    std::fs::write(local.join("socket.hnl"), "export socket := (x) ~> x + 100\n").unwrap();
    std::fs::write(vendored.join("socket.hnl"), "export socket := (x) ~> x + 1\n").unwrap();
    std::fs::write(
        project.join("consumer.hnl"),
        "from \"libc/socket\" import socket\nexport go := (x) ~> socket(x)\n",
    )
    .unwrap();

    let deps = HashMap::from([("libc".to_string(), vendored)]);
    let (_loader, rt) = loader_and_runtime(project.clone(), deps);

    let value = rt
        .run(&Source::new(
            r#"do { from "consumer.hnl" import go; go(5) }"#,
        ))
        .expect("the dep name must win over the sibling directory");
    assert_eq!(value, json!(6), "dep `libc` shadows `./libc`");

    std::fs::remove_dir_all(&root).ok();
}

/// The git-package→dir bridge, end to end: the fixture project commits a
/// `.handl.toml` that names the dep portably —
/// `host-tensor = { git = "…", tag = "v0.1" }` — and a `[patch]` entry
/// dev-links the name to a sibling checkout (the Cargo `[patch]` story).
/// `resolve_dependencies` maps the name to the sibling dir and
/// `from "host-tensor" import tensor` resolves through the loader.
#[test]
fn patch_bridges_a_git_dependency_to_a_sibling_checkout() {
    let root = scratch_dir("patch");
    let project = root.join("project");
    let dep = root.join("lib-host-tensor");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&dep).unwrap();

    // The package dir the sibling checkout stands in for: an
    // `index.hnl` exporting a `tensor` record of arrow fields.
    std::fs::write(
        dep.join("index.hnl"),
        "export tensor := { add: (a, b) ~> a + b }\n",
    )
    .unwrap();
    std::fs::write(
        project.join(".handl.toml"),
        concat!(
            "[dependencies]\n",
            "host-tensor = { git = \"https://github.com/handl-lang/lib-host-tensor\", tag = \"v0.1\" }\n",
            "\n[patch]\n",
            "host-tensor = \"../lib-host-tensor\"\n",
        ),
    )
    .unwrap();

    // The manifest half of the bridge: git source + patch → sibling dir.
    let deps =
        handl_platform::resolve_dependencies(&project).expect("the fixture manifest resolves");
    assert_eq!(
        deps["host-tensor"],
        std::fs::canonicalize(&dep).unwrap(),
        "[patch] must win over the git source"
    );

    let (_loader, rt) = loader_and_runtime(project.clone(), deps);
    let value = rt
        .run(&Source::new(
            r#"do { from "host-tensor" import tensor; tensor.add(2, 3) }"#,
        ))
        .expect("`from \"host-tensor\" import tensor` resolves via the patch");
    assert_eq!(value, json!(5));

    std::fs::remove_dir_all(&root).ok();
}

/// The missing-sibling half of the bridge: a committed `[patch]` whose
/// directory does not exist is inert — a git dep then resolves to its
/// `~/.handl/pkg/<name>` package-cache slot (the dir the fetch hook,
/// follow-up seam, populates) rather than failing manifest resolution.
#[test]
fn an_inert_patch_leaves_the_git_dep_at_its_cache_slot() {
    let root = scratch_dir("inert-patch");
    let project = root.join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join(".handl.toml"),
        concat!(
            "[dependencies]\n",
            "host-tensor = { git = \"https://github.com/handl-lang/lib-host-tensor\" }\n",
            "\n[patch]\n",
            "host-tensor = \"../lib-host-tensor\"\n", // does not exist
        ),
    )
    .unwrap();

    let deps = handl_platform::resolve_dependencies(&project).unwrap();
    assert_eq!(
        deps["host-tensor"],
        handl_platform::package_cache_dir().join("host-tensor"),
        "missing patch dir → the dep stays at its package-cache slot"
    );

    std::fs::remove_dir_all(&root).ok();
}
