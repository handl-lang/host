//! Phase 9.5 integration tests: `import` / `export` engine handling and the
//! handl-source module loader.
//!
//! These exercise the full stack through [`handl_kit_canonical::Kit`]: the
//! `import` native op delegates to the runtime's [`ImportResolver`] (a
//! `ModuleLoader`), which reads, lexes, parses, evaluates, and caches
//! handl-source modules. `export` is a bootstrap pass-through.

use std::fs;
use std::path::PathBuf;

use handl_kit_canonical::Kit;
use serde_json::json;

/// Write `contents` to `<dir>/<name>` and return the full path.
fn write_module(dir: &std::path::Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).expect("write module file");
    path
}

/// `import "module.hnl"` loads the module and binds its top-level bindings
/// (`x = 42`) into the importer's scope. Because the Pratt parser parses
/// one top-level statement, the import is wrapped in a block whose final
/// expression references the imported binding.
#[test]
fn import_loads_module_bindings() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "module.hnl", "x :: 42");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    // `do { import "module.hnl"; x }` — import binds `x` into the block
    // scope, then `x` evaluates to 42. (A bare `{...}` is an object literal;
    // `do` introduces a statement block.)
    let val = kit
        .eval("do { import \"module.hnl\"; x }")
        .expect("eval should succeed");
    assert_eq!(val, json!(42));
}

/// `import "nonexistent.hnl"` errors with a clear message.
#[test]
fn import_missing_file_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let err = kit
        .eval("do { import \"nonexistent.hnl\"; x }")
        .expect_err("missing import should error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("nonexistent.hnl") || msg.contains("cannot read module"),
        "error should mention the missing module: {}",
        msg
    );
}

/// Importing the same module twice only evaluates it once: the loader caches
/// the bindings by canonical path. Verified by deleting the module file
/// after the first import — the second import still succeeds because the
/// cached bindings are returned without re-reading the file.
#[test]
fn import_caches_modules() {
    let dir = tempfile::tempdir().expect("tempdir");
    let module_path = write_module(dir.path(), "cached.hnl", "x :: 7");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    // First import loads + caches the module.
    let v1 = kit
        .eval("do { import \"cached.hnl\"; x }")
        .expect("first import should succeed");
    assert_eq!(v1, json!(7));

    // Delete the module file. A second import must still succeed because the
    // loader's cache returns the previously evaluated bindings without
    // re-reading the (now missing) file.
    fs::remove_file(&module_path).expect("remove module file");
    let v2 = kit
        .eval("do { import \"cached.hnl\"; x }")
        .expect("second import should hit the cache");
    assert_eq!(v2, json!(7));
}

/// Circular imports are detected and error gracefully. `a.hnl` imports
/// `b.hnl`, which imports `a.hnl` — the loader's "currently loading" set
/// detects the cycle.
#[test]
fn import_detects_circular_imports() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "a.hnl", "import \"b.hnl\"");
    write_module(dir.path(), "b.hnl", "import \"a.hnl\"");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let err = kit
        .eval("import \"a.hnl\"")
        .expect_err("circular import should error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("circular"),
        "error should mention circular import: {}",
        msg
    );
}

/// A filesystem-imported module with several private declarations and
/// several exports must evaluate to the *complete* export record — not
/// just the first top-level binding. `ModuleLoader` wraps the prepared
/// `privates; { exports }` body in `do { … }` (the same trick `hnl run`
/// uses), so every declaration runs in one scope.
#[test]
fn import_multi_export_module_binds_full_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write_module(
        root,
        "multi.hnl",
        concat!(
            "helper := (x) ~> x * 2\n",
            "seed := 21\n",
            "export a := 1\n",
            "export b := helper(seed)\n",
            "export c := \"three\"\n",
        ),
    );
    let main = write_module(
        root,
        "main.hnl",
        "do { from \"./multi\" import * as m; { a: m.a, b: m.b, c: m.c } }",
    );

    let kit = Kit::discover(&main).expect("kit");
    let src = fs::read_to_string(&main).expect("read main.hnl");
    let val = kit.eval(&src).expect("eval should succeed");
    assert_eq!(val, json!({ "a": 1, "b": 42, "c": "three" }));
}

/// The same completeness guarantee through named projection:
/// `from "m" import { b }` must see every export, not only the first
/// binding of the module body.
#[test]
fn import_multi_export_named_projection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write_module(
        root,
        "multi.hnl",
        concat!(
            "helper := (x) ~> x * 2\n",
            "export a := 1\n",
            "export b := helper(21)\n",
        ),
    );
    let main = write_module(root, "main.hnl", "do { from \"./multi\" import { b }; b }");

    let kit = Kit::discover(&main).expect("kit");
    let src = fs::read_to_string(&main).expect("read main.hnl");
    let val = kit.eval(&src).expect("eval should succeed");
    assert_eq!(val, json!(42));
}

/// `export <expr>` is a bootstrap pass-through: it evaluates the child
/// expression and returns it.
#[test]
fn export_passes_through() {
    let dir = tempfile::tempdir().expect("tempdir");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit.eval("export 42").expect("eval should succeed");
    assert_eq!(val, json!(42));

    // Pass-through of a non-trivial expression.
    let val2 = kit.eval("export 6 * 7").expect("eval should succeed");
    assert_eq!(val2, json!(42));
}
// ---------------------------------------------------------------------------
// `.handl.toml` `[dependencies]` — project dependency resolution (Phase 9.5)
// ---------------------------------------------------------------------------

/// `import "arrows/transform/map.hnl"` resolves inside the `arrows` dependency's
/// directory declared in the project manifest's `[dependencies]` table.
///
/// Fixture layout (built in a tempdir, mirroring `hnl run`):
///
/// ```text
/// <root>/.handl.toml               [dependencies] arrows = "vendor/arrows"
/// <root>/vendor/arrows/transform/map.hnl  double :: x ~> x * 2
/// <root>/main.hnl                  import "arrows/transform/map.hnl" ... double(21)
/// ```
///
/// `Kit::discover` finds the manifest from the entry file's directory and
/// wires the dep map into the `ModuleLoader`.
#[test]
fn import_resolves_handl_toml_dependencies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    fs::write(
        root.join(".handl.toml"),
        "[dependencies]\narrows = \"vendor/arrows\"\n",
    )
    .expect("write manifest");
    let dep_dir = root.join("vendor/arrows/transform");
    fs::create_dir_all(&dep_dir).expect("create dep dir");
    write_module(&dep_dir, "map.hnl", "double :: x ~> x * 2");
    let main = write_module(
        root,
        "main.hnl",
        "do { import \"arrows/transform/map.hnl\"; double(21) }",
    );

    let kit = Kit::discover(&main).expect("kit");
    let src = fs::read_to_string(&main).expect("read main.hnl");
    let val = kit.eval(&src).expect("eval should succeed");
    assert_eq!(val, json!(42));
}

/// A `[dependencies]` table entry in `{ path = "..." }` table form resolves
/// the same way as the bare-string form.
#[test]
fn import_resolves_table_form_dependencies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    fs::write(
        root.join(".handl.toml"),
        "[dependencies]\nutil = { path = \"vendor/util\" }\n",
    )
    .expect("write manifest");
    let dep_dir = root.join("vendor/util");
    fs::create_dir_all(&dep_dir).expect("create dep dir");
    write_module(&dep_dir, "answer.hnl", "answer :: 42");
    let main = write_module(
        root,
        "main.hnl",
        "do { import \"util/answer.hnl\"; answer }",
    );

    let kit = Kit::discover(&main).expect("kit");
    let src = fs::read_to_string(&main).expect("read main.hnl");
    let val = kit.eval(&src).expect("eval should succeed");
    assert_eq!(val, json!(42));
}

/// Nested relative imports resolve against the importing module's own
/// directory: `vendor/arrows/transform/map.hnl` can `import "../consts.hnl"` to
/// reach `vendor/arrows/consts.hnl`.
#[test]
fn import_nested_relative_resolves_against_module_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    fs::write(
        root.join(".handl.toml"),
        "[dependencies]\narrows = \"vendor/arrows\"\n",
    )
    .expect("write manifest");
    let dep_dir = root.join("vendor/arrows/transform");
    fs::create_dir_all(&dep_dir).expect("create dep dir");
    // `map.hnl` must keep its export as the single root statement (the
    // Pratt parser yields one top-level node per module), so the nested
    // import happens inside the bound `do` block and `two` is captured
    // into the exported arrow's env.
    write_module(&root.join("vendor/arrows"), "consts.hnl", "two :: 2");
    write_module(
        &dep_dir,
        "map.hnl",
        "double :: do { import \"../consts.hnl\"; x ~> x * two }",
    );
    let main = write_module(
        root,
        "main.hnl",
        "do { import \"arrows/transform/map.hnl\"; double(21) }",
    );

    let kit = Kit::discover(&main).expect("kit");
    let src = fs::read_to_string(&main).expect("read main.hnl");
    let val = kit.eval(&src).expect("eval should succeed");
    assert_eq!(val, json!(42));
}

/// Without a manifest (or without a matching `[dependencies]` entry), an
/// import path falls back to module-root-relative resolution and errors
/// clearly when the file does not exist.
#[test]
fn import_dependency_fallback_to_module_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Plain relative import still works with no `.handl.toml`.
    write_module(dir.path(), "local.hnl", "x :: 7");
    let kit = Kit::discover(dir.path()).expect("kit");
    let val = kit
        .eval("do { import \"local.hnl\"; x }")
        .expect("relative import should succeed");
    assert_eq!(val, json!(7));

    // A path that looks like a dep specifier but matches nothing resolves
    // module-root-relative and fails with a read error naming the path.
    let err = kit
        .eval("do { import \"arrows/transform/map.hnl\"; x }")
        .expect_err("undeclared dep path should error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("arrows/transform/map.hnl") || msg.contains("cannot read module"),
        "error should mention the unresolved path: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// `from "path" import ...` — the full canonical import surface (ADR-0096)
// ---------------------------------------------------------------------------

/// `from "mod.hnl" import *` splices the module's exports into scope — the
/// same binding mode as bare `import "mod.hnl"`.
#[test]
fn from_import_star_splices_bindings() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "mod.hnl", "x :: 42");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit
        .eval("do { from \"mod.hnl\" import *; x }")
        .expect("from-import * should splice bindings");
    assert_eq!(val, json!(42));
}

/// `from "mod.hnl" import * as m` binds the whole module bindings object
/// to `m`.
#[test]
fn from_import_star_as_alias_binds_module_object() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "mod.hnl", "x :: 42");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit
        .eval("do { from \"mod.hnl\" import * as m; m }")
        .expect("from-import * as should bind the module object");
    assert_eq!(val, json!({"x": 42}));
}

/// `from "mod.hnl" import name` projects a single export and binds it
/// under its own name.
#[test]
fn from_import_named_binds_single_export() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "mod.hnl", "double :: (x) ~> x * 2");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit
        .eval("do { from \"mod.hnl\" import double; double(21) }")
        .expect("named import should bind the export");
    assert_eq!(val, json!(42));
}

/// `from "mod.hnl" import name as local` binds the projected export under
/// the alias.
#[test]
fn from_import_named_as_alias() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "mod.hnl", "double :: (x) ~> x * 2");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit
        .eval("do { from \"mod.hnl\" import double as d; d(21) }")
        .expect("named import with alias should bind the alias");
    assert_eq!(val, json!(42));
}

/// A named import for a missing export errors with a clear message.
#[test]
fn from_import_missing_export_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "mod.hnl", "x :: 42");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let err = kit
        .eval("do { from \"mod.hnl\" import missing; missing }")
        .expect_err("missing export should error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("no export `missing`"),
        "error should name the missing export: {}",
        msg
    );
}

/// The `as`-gated alias tail must not swallow a following statement's
/// leading identifier: `from "m" import name` on its own line followed by
/// a statement starting with an identifier keeps the two statements
/// separate.
#[test]
fn from_import_alias_does_not_swallow_next_statement() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "mod.hnl", "x :: 7");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit
        .eval("do { from \"mod.hnl\" import x\nx + 1 }")
        .expect("next statement must not be consumed as alias");
    assert_eq!(val, json!(8));
}

/// A file-loaded module may carry host-module declaration headers
/// (`from "str" import * as str`): the specifier names a module resolved
/// via the shared module chain, so the loader strips the line and the
/// module body's `str.*` calls still dispatch.
#[test]
fn nested_module_with_host_module_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(
        dir.path(),
        "inner.hnl",
        "from \"str\" import * as str\nx :: str.trim(\"  hi  \")",
    );
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit
        .eval("do { import \"inner.hnl\"; x }")
        .expect("module with host-module header should load");
    assert_eq!(val, json!("hi"));
}

/// `import <name> ...` declaration lines inside a file-loaded module are
/// stripped (the canonical grammar cannot execute the name-first order).
#[test]
fn nested_module_with_name_first_decl_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_module(dir.path(), "inner.hnl", "import str from \"str\"\nx :: 5");
    let kit = Kit::new_with_module_root(dir.path().to_path_buf()).expect("kit");

    let val = kit
        .eval("do { import \"inner.hnl\"; x }")
        .expect("module with name-first decl header should load");
    assert_eq!(val, json!(5));
}

/// Closure capture must be limited to the names a body actually
/// references. Before the free-variable filter, `closure` minting
/// snapshotted the *entire* scope into every arrow's env, so a module
/// with N sequential function declarations produced O(2^N) nested env
/// structure — the module-boundary `tagged_to_value` conversion then
/// expanded it and the process was OOM-killed. A module exporting a
/// closure after ~30 sibling closures would not survive this test
/// without the filter.
#[test]
fn import_module_with_many_closures_does_not_explode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    // `helper` exercises cross-closure capture (f_i references helper);
    // `v_i` exercises scalar capture.
    let mut module = String::from("helper := (x) ~> x * 2\n");
    for i in 0..30 {
        module.push_str(&format!("v{i} := {i}\n"));
        module.push_str(&format!("f{i} := (x) ~> helper(x) + v{i}\n"));
    }
    module.push_str("export head_fn := f0\nexport tail_fn := f29\n");
    write_module(root, "closures.hnl", &module);

    let main = write_module(
        root,
        "main.hnl",
        "do { from \"./closures\" import * as m; { a: m.head_fn(1), b: m.tail_fn(1) } }",
    );

    let kit = Kit::discover(&main).expect("kit");
    let src = fs::read_to_string(&main).expect("read main.hnl");
    let val = kit.eval(&src).expect("eval should succeed");
    // head_fn: helper(1) + v0 = 2 + 0; tail_fn: helper(1) + v29 = 2 + 29.
    assert_eq!(val, json!({ "a": 2, "b": 31 }));
}

/// An imported closure must retain access to its module-private bindings
/// after crossing the module boundary — the captured env is serialized
/// and reconstructed, so this fails if private captures are dropped.
#[test]
fn imported_closure_sees_module_privates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write_module(
        root,
        "priv.hnl",
        concat!(
            "offset := 100\n",
            "bump := (x) ~> x + offset\n",
            "export go := (x) ~> bump(x)\n",
        ),
    );
    let main = write_module(
        root,
        "main.hnl",
        "do { from \"./priv\" import * as m; m.go(1) }",
    );

    let kit = Kit::discover(&main).expect("kit");
    let src = fs::read_to_string(&main).expect("read main.hnl");
    let val = kit.eval(&src).expect("eval should succeed");
    assert_eq!(val, json!(101));
}
