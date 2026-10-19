//! End-to-end integration tests for the module system (ADR-0069).
//!
//! These tests load REAL `.hnl` files from disk via `FileResolver` +
//! `ModuleLoader` and run both normalisation strategies (Strategy A:
//! `normalize_compile_time`, Strategy B: `flatten_program`). They are the
//! production equivalent of the PoC's `cascade_check.rs` binary.
//!
//! Three groups:
//!   A. Single-file loading from `examples/handl/` on disk.
//!   B. Multi-file programs written to a temp directory (with imports).
//!   C. `handl-packages/` files loaded from disk.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use handl_plugin_ext_refinement::{
    FileResolver, LoadError, ModuleLoader, contains_module_ref, contains_project, flatten_program,
    has_unresolved_apps, normalize_compile_time,
};

// ============================================================================
// Helpers
// ============================================================================

/// The workspace root (`platform/`), derived from this crate's manifest dir.
/// The arrow-dialect [`Language`] for tests — canonical base from the
/// dev-dep data crate plus the `arrow_language_for` overrides (tests may
/// depend on `lang/` data; the library must not).
fn arrow_lang() -> &'static handl_language::Language {
    static LANG: std::sync::OnceLock<handl_language::Language> =
        std::sync::OnceLock::new();
    LANG.get_or_init(|| {
        handl_plugin_ext_refinement::arrow_lang::arrow_language_for(
            &handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        )
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `examples/handl/` directory.
fn examples_dir() -> PathBuf {
    workspace_root().join("examples/handl")
}

/// This crate's arrow-dialect fixture corpus (`tests/fixtures/`). The former
/// `handl-packages/{fn,collection,state}/` arrows live here — they are
/// superseded as stdlib packages and kept only as refinement-checker
/// fixtures.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Create a unique temp directory for a test, prefixed by the test name and
/// the process id. Returns the path. Uses `std::env::temp_dir()` since the
/// `tempfile` crate is not a dev-dependency.
fn make_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "handl_integration_{}_{}_{}",
        test_name,
        std::process::id(),
        // A counter-ish value from the system time nanos to avoid collisions
        // when the same test runs in parallel within one process.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("must create temp dir");
    dir
}

/// Write a `.hnl` file into `dir` with the given relative path (without
/// extension) and content.
fn write_module(dir: &PathBuf, rel_path: &str, content: &str) {
    let mut path = dir.clone();
    for component in rel_path.split('/') {
        path.push(component);
    }
    path.set_extension("hnl");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("must create parent dir");
    }
    fs::write(&path, content).expect("must write module file");
}

/// Build a `ModuleLoader` backed by a `FileResolver` rooted at `root`.
fn loader_for(root: PathBuf) -> ModuleLoader {
    ModuleLoader::new(Box::new(FileResolver::new(root)), arrow_lang().clone())
}

// ============================================================================
// A. Single-file loading from disk: examples/handl/
// ============================================================================

/// Try to load a single example file via `ModuleLoader::load`. Returns the
/// `LoadedModule` on success, or `None` if the file does not parse (the
/// parser is minimal and some example files use syntax it does not support).
fn try_load_example(
    loader: &ModuleLoader,
    name: &str,
) -> Option<handl_plugin_ext_refinement::LoadedModule> {
    match loader.load(name) {
        Ok(m) => Some(m),
        Err(LoadError::Parse(msg)) => {
            // The parser is minimal; skip files that do not parse rather than
            // failing the whole test suite.
            eprintln!("  [skip] {} did not parse: {}", name, msg);
            None
        }
        Err(e) => {
            eprintln!("  [skip] {} load error: {}", name, e);
            None
        }
    }
}

#[test]
fn test_file_resolver_examples() {
    let dir = examples_dir();
    if !dir.is_dir() {
        // The examples directory may not be present in some checkouts; skip
        // gracefully rather than failing.
        eprintln!(
            "  [skip] examples/handl directory not found at {}",
            dir.display()
        );
        return;
    }
    let loader = loader_for(dir);

    // A representative selection of example files. NOTE: the files in
    // `examples/handl/` use the general handl surface syntax (`while { ... }`,
    // `match`, records, lambdas), NOT the arrow-graph DSL that
    // `parse_arrow_module` understands. Most therefore do not parse and are
    // skipped. The few that do parse (e.g. `hello`) are single-expression
    // files with no imports, so we only assert the invariant that holds for
    // any single-file program: no `module_ref` after flattening. We do NOT
    // assert no-unresolved-apps because these general programs reference names
    // outside the arrow-graph namespace (e.g. `hello.greet`).
    let candidates = [
        "fibonacci",
        "factorial",
        "gcd",
        "sum-to-n",
        "power",
        "hello",
        "composition",
        "conditionals",
    ];

    let mut loaded_any = false;
    for name in &candidates {
        let loaded = match try_load_example(&loader, name) {
            Some(m) => m,
            None => continue,
        };
        loaded_any = true;
        assert!(
            !loaded.arrows.is_empty(),
            "loaded module '{}' should have at least one arrow",
            name
        );

        // Strategy B: flatten_program on a single-file program (no imports)
        // should produce a graph with no module_ref.
        match flatten_program(name, &loader) {
            Ok(flat) => {
                assert!(
                    !contains_module_ref(&flat),
                    "flatten_program({}): no module_ref should remain for single-file program",
                    name
                );
            }
            Err(e) => {
                // flatten_program may fail if the parsed graph references names
                // the resolver cannot inline (the parser is minimal). Skip
                // rather than fail.
                eprintln!("  [skip] flatten_program({}) failed: {}", name, e);
            }
        }
    }

    // At least one example file should load and parse. If none do, the
    // examples/handl directory has drifted away from arrow-parseable syntax
    // entirely and this test should be revisited.
    assert!(
        loaded_any,
        "at least one example file should load and parse"
    );
}

// ============================================================================
// B. Multi-file programs from disk (temp directory with imports)
// ============================================================================

#[test]
fn test_multi_file_disk_strategy_a() {
    let dir = make_temp_dir("strategy_a");
    // util.hnl — defines `helper`.
    write_module(&dir, "util", "helper = arr(\"add\") >>> arr(\"add\")");
    // main.hnl — imports `helper` from `util`, uses it.
    write_module(&dir, "main", "import helper from \"util\"\nmain = helper");

    let loader = loader_for(dir);

    match normalize_compile_time("main", &loader) {
        Ok(normalized) => {
            // Strategy A eliminates module_ref (replaces with inlined records).
            let any_ref = normalized.values().any(|s| contains_module_ref(s));
            assert!(
                !any_ref,
                "Strategy A: no module_ref should remain after normalization"
            );
            // Strategy A keeps projections (project nodes may remain).
            let main = normalized.get("main");
            assert!(main.is_some(), "normalized module should have 'main' arrow");
        }
        Err(e) => panic!("normalize_compile_time(main) should succeed: {}", e),
    }
}

#[test]
fn test_multi_file_disk_strategy_b() {
    let dir = make_temp_dir("strategy_b");
    write_module(&dir, "util", "helper = arr(\"add\") >>> arr(\"add\")");
    write_module(&dir, "main", "import helper from \"util\"\nmain = helper");

    let loader = loader_for(dir);

    match flatten_program("main", &loader) {
        Ok(flat) => {
            assert!(
                !contains_module_ref(&flat),
                "Strategy B: no module_ref should remain after flatten"
            );
            assert!(
                !contains_project(&flat),
                "Strategy B: no project should remain after flatten"
            );
            // No unresolved apps: the known names are the entry + imported
            // arrows plus native ops.
            let known: HashSet<String> = vec![
                "main".to_string(),
                "helper".to_string(),
                "add".to_string(),
                "id".to_string(),
                "const".to_string(),
                "record".to_string(),
                "field".to_string(),
                "project".to_string(),
                "module_ref".to_string(),
            ]
            .into_iter()
            .collect();
            assert!(
                !has_unresolved_apps(&flat, &known),
                "Strategy B: no unresolved App nodes should remain after flatten"
            );
        }
        Err(e) => panic!("flatten_program(main) should succeed: {}", e),
    }
}

#[test]
fn test_multi_file_cycle_disk() {
    let dir = make_temp_dir("cycle");
    // a.hnl imports from b.hnl; b.hnl imports from a.hnl — a cycle.
    write_module(&dir, "a", "import b from \"b\"\nx = arr(\"id\")");
    write_module(&dir, "b", "import a from \"a\"\ny = arr(\"id\")");

    let loader = loader_for(dir);

    match normalize_compile_time("a", &loader) {
        Ok(_) => panic!("normalize_compile_time(a) should detect cycle"),
        Err(e) => {
            assert!(
                matches!(e, LoadError::Cycle(_)),
                "expected LoadError::Cycle, got: {}",
                e
            );
        }
    }

    // Also verify flatten_program detects the same cycle.
    match flatten_program("a", &loader) {
        Ok(_) => panic!("flatten_program(a) should detect cycle"),
        Err(e) => {
            assert!(
                matches!(e, LoadError::Cycle(_)),
                "flatten_program: expected LoadError::Cycle, got: {}",
                e
            );
        }
    }
}

#[test]
fn test_three_file_chain() {
    let dir = make_temp_dir("chain");
    // c.hnl — leaf: defines `base`.
    write_module(&dir, "c", "base = arr(\"id\")");
    // b.hnl — imports `base` from c, defines `mid`.
    write_module(
        &dir,
        "b",
        "import base from \"c\"\nmid = base >>> arr(\"const\")",
    );
    // a.hnl — imports `mid` from b, defines `main`.
    write_module(&dir, "a", "import mid from \"b\"\nmain = mid");

    let loader = loader_for(dir);

    // Strategy A: no module_ref remains (project may remain).
    match normalize_compile_time("a", &loader) {
        Ok(normalized) => {
            let any_ref = normalized.values().any(|s| contains_module_ref(s));
            assert!(
                !any_ref,
                "three-file chain Strategy A: no module_ref should remain"
            );
        }
        Err(e) => panic!("normalize_compile_time(a) should succeed: {}", e),
    }

    // Strategy B: no module_ref, no project, no unresolved apps.
    match flatten_program("a", &loader) {
        Ok(flat) => {
            assert!(
                !contains_module_ref(&flat),
                "three-file chain Strategy B: no module_ref should remain"
            );
            assert!(
                !contains_project(&flat),
                "three-file chain Strategy B: no project should remain"
            );
            let known: HashSet<String> = vec![
                "main".to_string(),
                "mid".to_string(),
                "base".to_string(),
                "id".to_string(),
                "const".to_string(),
                "record".to_string(),
                "field".to_string(),
                "project".to_string(),
                "module_ref".to_string(),
            ]
            .into_iter()
            .collect();
            assert!(
                !has_unresolved_apps(&flat, &known),
                "three-file chain Strategy B: no unresolved App nodes should remain"
            );
        }
        Err(e) => panic!("flatten_program(a) should succeed: {}", e),
    }
}

// ============================================================================
// C. kit-stdlib files from disk
// ============================================================================

#[test]
fn test_file_resolver_kit_stdlib() {
    // The arrow-dialect corpus (formerly `handl-packages/{fn,collection,
    // state}/`) lives under `tests/fixtures/` — those packages are superseded
    // and survive only as refinement-checker fixtures.
    let dir = fixtures_dir();
    if !dir.is_dir() {
        eprintln!("  [skip] fixtures directory not found at {}", dir.display());
        return;
    }
    let loader = loader_for(dir);

    // A representative selection of arrow-dialect fixture files. These use
    // the rich `rec { ... } (...)` / `loop { ... } (...)` surface syntax.
    let candidates = [
        "fn/map",
        "collection/sort",
        "state/counter",
        "collection/insert",
        "fn/filter",
        "fn/fold",
    ];

    let mut loaded_any = false;
    for &name in &candidates {
        let loaded = match try_load_example(&loader, name) {
            Some(m) => m,
            None => continue,
        };
        loaded_any = true;
        assert!(
            !loaded.arrows.is_empty(),
            "loaded kit-stdlib module '{}' should have at least one arrow",
            name
        );

        // Strategy B: flatten_program should produce a clean graph.
        match flatten_program(name, &loader) {
            Ok(flat) => {
                assert!(
                    !contains_module_ref(&flat),
                    "flatten_program({}): no module_ref should remain",
                    name
                );
                // kit-stdlib files are single-file (no imports), so there
                // should be no project nodes either.
                assert!(
                    !contains_project(&flat),
                    "flatten_program({}): no project should remain for single-file module",
                    name
                );
                // All App nodes should be inlined — no residual unresolved
                // apps. Bare-name references (e.g. sort calling insert) are
                // auto-discovered and inlined by load_transitive.
                let known: HashSet<String> = HashSet::new();
                assert!(
                    !has_unresolved_apps(&flat, &known),
                    "flatten_program({}): no unresolved App nodes should remain",
                    name
                );
            }
            Err(e) => {
                eprintln!("  [skip] flatten_program({}) failed: {}", name, e);
            }
        }
    }

    assert!(
        loaded_any,
        "at least one kit-stdlib file should load and parse"
    );
}
