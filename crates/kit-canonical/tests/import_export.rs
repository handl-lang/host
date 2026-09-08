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

/// `import "arrows/fn/map.hnl"` resolves inside the `arrows` dependency's
/// directory declared in the project manifest's `[dependencies]` table.
///
/// Fixture layout (built in a tempdir, mirroring `hnl run`):
///
/// ```text
/// <root>/.handl.toml               [dependencies] arrows = "vendor/arrows"
/// <root>/vendor/arrows/fn/map.hnl  double :: x ~> x * 2
/// <root>/main.hnl                  import "arrows/fn/map.hnl" ... double(21)
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
    let dep_dir = root.join("vendor/arrows/fn");
    fs::create_dir_all(&dep_dir).expect("create dep dir");
    write_module(&dep_dir, "map.hnl", "double :: x ~> x * 2");
    let main = write_module(
        root,
        "main.hnl",
        "do { import \"arrows/fn/map.hnl\"; double(21) }",
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
/// directory: `vendor/arrows/fn/map.hnl` can `import "../consts.hnl"` to
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
    let dep_dir = root.join("vendor/arrows/fn");
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
        "do { import \"arrows/fn/map.hnl\"; double(21) }",
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
        .eval("do { import \"arrows/fn/map.hnl\"; x }")
        .expect_err("undeclared dep path should error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("arrows/fn/map.hnl") || msg.contains("cannot read module"),
        "error should mention the unresolved path: {}",
        msg
    );
}
