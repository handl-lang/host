//! # On-disk object-file compilation integration tests (ADR-0044 Phase 8)
//!
//! These tests exercise the new [`CompileMode::Object`] / [`compile_all_nodes_object`]
//! path end-to-end: every case in the shared eval conformance corpus (from
//! `handl-test-utils`) is compiled to a relocatable `.o` file, and the file is
//! asserted to exist and be non-empty.
//!
//! ## Scope
//!
//! Linking + running the emitted object file is **out of scope** for this
//! task — the `.o` file is the deliverable. The object file declares the
//! `helper_*` functions as external imports; a future linker step will link
//! the object against a runtime stub library that exports those symbols (the
//! `extern "C"` helpers in [`handl_plugin_engine_eval_cranelift`]) plus a
//! small `main` driver, producing a standalone executable. This file only
//! verifies that compilation to a valid relocatable object succeeds for the
//! full corpus.

use std::path::PathBuf;

use handl_plugin_engine_eval_cranelift::{CompileMode, CraneliftEngine, compile_all_nodes_object};
use handl_test_utils::conformance::eval_corpus;

/// A unique temp path for a test object file, derived from the case name.
///
/// Files are written under [`std::env::temp_dir`] and removed at the end of
/// each test (best-effort).
fn temp_object_path(case_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("handl_object_compilation_{}.o", case_name))
}

/// Every case in the eval conformance corpus compiles to a non-empty `.o`
/// file via [`compile_all_nodes_object`].
#[test]
fn corpus_compiles_to_nonempty_object_files() {
    for case in eval_corpus() {
        let path = temp_object_path(case.name);
        let _ = std::fs::remove_file(&path);

        let result = compile_all_nodes_object(&case.lang, &case.ast, &path);
        assert!(
            result.is_ok(),
            "case '{}': object compilation failed: {:?}",
            case.name,
            result.err()
        );

        assert!(
            path.exists(),
            "case '{}': object file was not created at {}",
            case.name,
            path.display()
        );

        let metadata = std::fs::metadata(&path).expect("object file metadata must be readable");
        assert!(
            metadata.len() > 0,
            "case '{}': object file is empty",
            case.name
        );

        let _ = std::fs::remove_file(&path);
    }
}

/// The emitted object file is a recognized relocatable object (ELF on Linux,
/// Mach-O on macOS) — checked via magic bytes so the test is portable.
#[test]
fn corpus_emits_valid_object_magic_bytes() {
    let corpus = eval_corpus();
    // Pick a representative non-trivial case (the "add" case, or fall back to
    // the first case).
    let case = corpus
        .iter()
        .find(|c| c.name == "add")
        .or_else(|| corpus.first())
        .expect("corpus must contain at least one case");

    let path = temp_object_path("magic");
    let _ = std::fs::remove_file(&path);

    compile_all_nodes_object(&case.lang, &case.ast, &path)
        .expect("object compilation must succeed");

    let bytes = std::fs::read(&path).expect("object file must be readable");
    assert!(bytes.len() > 0, "object file must be non-empty");

    // ELF magic: `\x7fELF` (Linux). Mach-O magic: `0xfeedfacf` (64-bit) or
    // `0xfeedface` (32-bit) (macOS). Either is a valid relocatable object on
    // the respective host platform.
    let is_elf = bytes.len() >= 4 && &bytes[0..4] == b"\x7fELF";
    let is_macho = bytes.len() >= 4
        && (bytes[0..4] == [0xcf, 0xfa, 0xed, 0xfe] || bytes[0..4] == [0xce, 0xfa, 0xed, 0xfe]);
    assert!(
        is_elf || is_macho,
        "emitted file is not a recognized relocatable object (first 4 bytes: {:?})",
        &bytes.get(0..4).unwrap_or(&[])
    );

    let _ = std::fs::remove_file(&path);
}

/// [`CompileMode::Object`] constructs and round-trips through the unified
/// [`handl_plugin_engine_eval_cranelift::compile_all_nodes`] dispatcher,
/// producing a non-empty file with an empty function-pointer map.
#[test]
fn compile_mode_object_dispatch_produces_file() {
    use handl_plugin_engine_eval_cranelift::compile_all_nodes;

    let corpus = eval_corpus();
    let case = corpus
        .iter()
        .find(|c| c.name == "add")
        .or_else(|| corpus.first())
        .expect("corpus must contain at least one case");

    let path = temp_object_path("dispatch");
    let _ = std::fs::remove_file(&path);

    let mode = CompileMode::Object {
        output_path: path.clone(),
    };
    let result = compile_all_nodes(
        &case.lang,
        &case.ast,
        &mode,
        &std::collections::HashMap::new(),
    )
    .expect("dispatched object compilation must succeed");
    assert!(
        result.compiled.is_empty(),
        "object mode should produce no function pointers"
    );
    assert!(path.exists(), "object file should exist after dispatch");

    let _ = std::fs::remove_file(&path);
}

/// The [`CraneliftEngine::compile_to_object`] method (the ADR-0044 Phase 8
/// `compile(handlLanguage, native)` entry point) emits a non-empty `.o` file
/// for a representative corpus case.
#[test]
fn engine_compile_to_object_emits_file() {
    let corpus = eval_corpus();
    let case = corpus
        .iter()
        .find(|c| c.name == "add")
        .or_else(|| corpus.first())
        .expect("corpus must contain at least one case");

    let path = temp_object_path("engine");
    let _ = std::fs::remove_file(&path);

    let engine = CraneliftEngine::new();
    let result = engine.compile_to_object(&case.lang, &case.ast, &path);
    assert!(
        result.is_ok(),
        "engine.compile_to_object failed: {:?}",
        result.err()
    );
    assert!(
        path.exists(),
        "object file should exist after engine compile"
    );
    let metadata = std::fs::metadata(&path).expect("metadata readable");
    assert!(metadata.len() > 0, "object file should be non-empty");

    let _ = std::fs::remove_file(&path);
}

/// The object result carries per-node function names (`node_N`) for linking.
#[test]
fn object_result_carries_function_names() {
    let corpus = eval_corpus();
    let case = corpus
        .iter()
        .find(|c| c.name == "add")
        .or_else(|| corpus.first())
        .expect("corpus must contain at least one case");

    let path = temp_object_path("names");
    let _ = std::fs::remove_file(&path);

    let result = compile_all_nodes_object(&case.lang, &case.ast, &path)
        .expect("object compilation must succeed");
    assert!(
        !result.function_names.is_empty(),
        "function_names should be non-empty for a non-empty AST"
    );
    // Every entry should be `node_N` for some N.
    for (idx, name) in &result.function_names {
        assert_eq!(
            name,
            &format!("node_{}", idx),
            "function name should be node_{} but was {}",
            idx,
            name
        );
    }

    let _ = std::fs::remove_file(&path);
}
