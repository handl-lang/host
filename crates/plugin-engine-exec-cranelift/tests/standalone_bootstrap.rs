//! # Standalone binary bootstrap integration tests (ADR-0044 Phase 8)
//!
//! These tests exercise the full `compile(handlLanguage, native)` pipeline:
//! each case in the shared eval conformance corpus is compiled to a
//! **standalone executable** via
//! [`CraneliftEngine::compile_to_executable`], the executable is run, and
//! its stdout (JSON) is compared against the expected value.
//!
//! The standalone executable links the cranelift-emitted `.o` file (native
//! code for every node) with a generated C `main` driver and this crate's
//! static library (which exports the `helper_*` runtime stub and the
//! `handl_standalone_*` entry points). This is the ADR-0044 Phase 8
//! proof-of-concept: the binary runs **without** the Rust host process —
//! only the C runtime + the linked native code.
//!
//! ## Scope & limitations
//!
//! - The `foreign` corpus case is **skipped** — standalone mode has no host
//!   functions registry at runtime, so foreign `Arr` op dispatch cannot
//!   resolve. This is a documented Phase 8 limitation.
//! - The executable is linked with `cc` against the crate's static library;
//!   the test requires `cc` and `cargo` on `PATH`.

use std::path::PathBuf;
use std::process::Command;

use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_test_utils::conformance::eval_corpus;

/// A unique temp path for a standalone executable, derived from the case
/// name. Files are removed at the end of each test (best-effort).
fn temp_exe_path(case_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("handl_standalone_bootstrap_{}", case_name))
}

/// Run a standalone executable and return its stdout as a trimmed string.
fn run_exe(path: &PathBuf) -> (String, String, i32) {
    let output = Command::new(path)
        .output()
        .unwrap_or_else(|e| panic!("failed to run standalone exe at {}: {}", path.display(), e));
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

/// Every non-foreign case in the eval conformance corpus compiles to a
/// standalone executable that, when run, produces the expected result.
#[test]
fn corpus_compiles_and_runs_standalone() {
    for case in eval_corpus() {
        // Skip the foreign case — standalone mode has no host registry.
        if case.name == "foreign" {
            continue;
        }

        let exe = temp_exe_path(case.name);
        let _ = std::fs::remove_file(&exe);

        // Compile to standalone executable.
        let engine = CraneliftEngine::new();
        let result = engine.compile_to_executable(&case.lang, &case.ast, &exe);
        assert!(
            result.is_ok(),
            "case '{}': compile_to_executable failed: {:?}",
            case.name,
            result.err()
        );
        assert!(
            exe.exists(),
            "case '{}': executable was not created at {}",
            case.name,
            exe.display()
        );

        // Run the executable.
        let (stdout, stderr, code) = run_exe(&exe);
        assert_eq!(
            code, 0,
            "case '{}': executable exited with code {} (stderr: {})",
            case.name, code, stderr
        );

        // Parse the output as JSON and compare.
        let result_value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!(
                "case '{}': stdout is not valid JSON: '{}' (parse error: {})",
                case.name, stdout, e
            )
        });
        assert_eq!(
            result_value, case.expected,
            "case '{}': expected {}, got {}",
            case.name, case.expected, result_value
        );

        let _ = std::fs::remove_file(&exe);
    }
}

/// The "add" case (a representative non-trivial case) compiles and runs
/// standalone, producing `5`.
#[test]
fn add_case_standalone() {
    let corpus = eval_corpus();
    let case = corpus
        .iter()
        .find(|c| c.name == "add")
        .expect("corpus must contain 'add'");

    let exe = temp_exe_path("add_unit");
    let _ = std::fs::remove_file(&exe);

    let engine = CraneliftEngine::new();
    engine
        .compile_to_executable(&case.lang, &case.ast, &exe)
        .expect("compile_to_executable must succeed for 'add'");

    let (stdout, stderr, code) = run_exe(&exe);
    assert_eq!(
        code, 0,
        "add case exited with code {} (stderr: {})",
        code, stderr
    );
    assert_eq!(stdout, "5");

    let _ = std::fs::remove_file(&exe);
}

/// The "nested_let_add" case (the most complex non-foreign case) compiles
/// and runs standalone, producing `7`.
#[test]
fn nested_let_add_case_standalone() {
    let corpus = eval_corpus();
    let case = corpus
        .iter()
        .find(|c| c.name == "nested_let_add")
        .expect("corpus must contain 'nested_let_add'");

    let exe = temp_exe_path("nested_let_add_unit");
    let _ = std::fs::remove_file(&exe);

    let engine = CraneliftEngine::new();
    engine
        .compile_to_executable(&case.lang, &case.ast, &exe)
        .expect("compile_to_executable must succeed for 'nested_let_add'");

    let (stdout, stderr, code) = run_exe(&exe);
    assert_eq!(
        code, 0,
        "nested_let_add case exited with code {} (stderr: {})",
        code, stderr
    );
    assert_eq!(stdout, "7");

    let _ = std::fs::remove_file(&exe);
}
