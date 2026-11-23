//! Tests for the `hnl compile` request seam (ADR-0114 Phase 14).
//!
//! PL-004 T-101: the dispatcher dogfood tests went away with
//! `CompileDispatcher` — the kernel `Emitter` path is covered by
//! `ext-compile` and the `tool-codegen` suite.

use handl_language::Language;

use crate::CompileFileRequest;

#[test]
fn compile_file_request_builds_input_with_root_ref() {
    let req = CompileFileRequest {
        target: "js".to_string(),
        path: std::path::PathBuf::from("main.hnl"),
        output: None,
        source_map: false,
        debug: false,
        format: Some("esm".to_string()),
        to: None,
        emit: None,
    };
    let src = "x = 1";
    let ast = CompileFileRequest::build_ast(src);
    let lang = Language::new();
    let term = CompileFileRequest::root_term_for(&lang);
    let report = hnl_tool_typecheck::TypecheckReport::default();
    let input = req.build_input(src, &ast, &lang, &term, &report);
    assert_eq!(input.root.index(), 0);
    assert_eq!(input.ast.len(), 1);
    assert_eq!(input.language, lang);
}
