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
