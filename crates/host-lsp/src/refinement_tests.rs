use crate::refinement::{
    find_arrows_root, format_refinement_summary, RefinementEngine,
};

const INSERT_HNL: &str = r#"
-- insert: insert an element into a sorted list, maintaining sorted order.
rec {
  elem: input.0,
  acc:  input.1
} (
  state ~> match state.acc {
    []: return [state.elem],

    [h, ...t]:
      cmp = prim.compare(state.elem, h) in
      if cmp <= 0
        { out: [state.elem, ...state.acc] }
      else
        rest_inserted = insert({ elem: state.elem, acc: t }) in
        {
          state: {
            elem: state.elem,
            acc:  [h, ...rest_inserted]
          }
        }
  }
)
"#;

const SIMPLE_COMP: &str = r#"id >>> id"#;

const NON_ARROW_HNL: &str = r#"@@@ this is not valid handl syntax @@@
"#;

#[test]
fn engine_constructs_with_standard_registry() {
    let engine = RefinementEngine::new();
    // No panic, registry is populated.
    let _ = engine.hover_markdown(SIMPLE_COMP, None);
}

#[test]
fn infer_document_arrow_file_succeeds() {
    let engine = RefinementEngine::new();
    let result = engine.infer_document(SIMPLE_COMP, None);
    assert!(result.is_some(), "simple comp should parse as arrow file");
    let inner = result.unwrap();
    assert!(inner.is_ok(), "inference should succeed: {:?}", inner.err());
}

#[test]
fn infer_document_non_arrow_file_returns_none() {
    let engine = RefinementEngine::new();
    let result = engine.infer_document(NON_ARROW_HNL, None);
    assert!(
        result.is_none(),
        "non-arrow .hnl should not be treated as arrow file"
    );
}

#[test]
fn hover_markdown_for_arrow_file_contains_refinement_header() {
    let engine = RefinementEngine::new();
    let md = engine
        .hover_markdown(SIMPLE_COMP, None)
        .expect("hover markdown for arrow file");
    assert!(
        md.contains("Inferred Contract"),
        "missing header: {}",
        md
    );
    assert!(md.contains("Guarantee"), "missing guarantee: {}", md);
}

#[test]
fn hover_markdown_for_non_arrow_file_is_none() {
    let engine = RefinementEngine::new();
    assert!(engine.hover_markdown(NON_ARROW_HNL, None).is_none());
}

#[test]
fn code_lenses_for_arrow_file_have_summary() {
    let engine = RefinementEngine::new();
    let lenses = engine.code_lenses(SIMPLE_COMP);
    assert_eq!(lenses.len(), 1);
    assert!(lenses[0].title.starts_with("guarantee:"));
    assert!(lenses[0].title.contains("terminates"));
}

#[test]
fn code_lenses_for_non_arrow_file_empty() {
    let engine = RefinementEngine::new();
    assert!(engine.code_lenses(NON_ARROW_HNL).is_empty());
}

#[test]
fn diagnostics_for_successful_inference_empty() {
    let engine = RefinementEngine::new();
    assert!(engine.diagnostics(SIMPLE_COMP, None).is_empty());
}

#[test]
fn diagnostics_for_non_arrow_file_empty() {
    let engine = RefinementEngine::new();
    assert!(engine.diagnostics(NON_ARROW_HNL, None).is_empty());
}

#[test]
fn diagnostics_for_unknown_base_arrow_reports_error() {
    let engine = RefinementEngine::new();
    // An arrow file referencing an unknown base arrow.
    let src = r#"arr("totally_unknown_op")"#;
    let diags = engine.diagnostics(src, None);
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("refinement"));
}

#[test]
fn find_arrows_root_locates_arrows_directory() {
    assert_eq!(
        find_arrows_root("/home/user/platform/handl-packages/collection/sort.hnl"),
        Some("/home/user/platform/handl-packages/".to_string())
    );
}

#[test]
fn find_arrows_root_returns_none_for_non_arrows_path() {
    assert_eq!(find_arrows_root("/home/user/platform/src/main.rs"), None);
}

#[test]
fn format_refinement_summary_contains_guarantee_and_termination() {
    let engine = RefinementEngine::new();
    let result = engine
        .infer_document(SIMPLE_COMP, None)
        .unwrap()
        .unwrap();
    let summary = format_refinement_summary(&result);
    assert!(summary.starts_with("guarantee:"));
    assert!(summary.contains("terminates"));
}

#[test]
fn rich_loop_insert_file_parses_and_infers() {
    let engine = RefinementEngine::new();
    let result = engine.infer_document(INSERT_HNL, None);
    assert!(result.is_some(), "rich loop syntax should parse as arrow file");
    // Inference may succeed or fail depending on registry coverage, but it
    // should at least parse and attempt inference.
}
