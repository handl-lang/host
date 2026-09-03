//! Tests for `crate::inlay_hints`.

use crate::inlay_hints::inlay_hints;
use crate::refinement::RefinementEngine;

/// A simple map-style arrow file using the rich-loop `rec { state } ( body )`
/// syntax (v0.1: `rec` is an alias for `loop`). Three state fields and a
/// `[h, ...t]:` recursive branch.
///
/// The kit lexer (v0.1) recognises `rec` as a keyword, and `inlay_hints.rs`
/// scans for the text `rec` to locate state fields in arrow files.
const MAP_ARROW: &str = r#"rec {
  remaining: input.0,
  f:         input.1,
  acc:       []
} (
  state ~> match state.remaining {
    []: return state.acc,

    [h, ...t]:
      y = state.f(h) in
      {
        state: {
          remaining: t,
          f:         state.f,
          acc:       [y, ...state.acc]
        }
      }
  }
)
"#;

#[test]
fn inlay_hints_for_arrow_file_shows_field_types() {
    let engine = RefinementEngine::new();
    let hints = inlay_hints(MAP_ARROW, &engine, None);
    assert!(!hints.is_empty(), "expected inlay hints for arrow file");
    // At least one hint should be a field-type hint (label starts with ": ").
    let has_field_type = hints.iter().any(|h| {
        h["label"]
            .as_str()
            .map(|l| l.starts_with(": "))
            .unwrap_or(false)
    });
    assert!(
        has_field_type,
        "expected a field-type hint among: {:?}",
        hints
    );
}

#[test]
fn inlay_hints_for_arrow_file_shows_rec_signature() {
    let engine = RefinementEngine::new();
    let hints = inlay_hints(MAP_ARROW, &engine, None);
    assert!(!hints.is_empty(), "expected inlay hints for arrow file");
    // The rec signature hint label contains `~>` (In ~> Out).
    let has_signature = hints.iter().any(|h| {
        h["label"]
            .as_str()
            .map(|l| l.contains("~>"))
            .unwrap_or(false)
    });
    assert!(
        has_signature,
        "expected a rec signature hint containing `~>` among: {:?}",
        hints
    );
}

#[test]
fn inlay_hints_for_non_arrow_file_returns_empty() {
    let engine = RefinementEngine::new();
    let hints = inlay_hints("x = 42", &engine, None);
    assert!(
        hints.is_empty(),
        "non-arrow .hnl should produce no inlay hints: {:?}",
        hints
    );
}

#[test]
fn inlay_hints_empty_source_returns_empty() {
    let engine = RefinementEngine::new();
    let hints = inlay_hints("", &engine, None);
    assert!(hints.is_empty(), "empty source should produce no hints");
}

#[test]
fn inlay_hints_returns_valid_json() {
    let engine = RefinementEngine::new();
    let hints = inlay_hints(MAP_ARROW, &engine, None);
    assert!(!hints.is_empty(), "expected inlay hints for arrow file");
    for hint in &hints {
        assert!(hint.get("position").is_some(), "hint missing position: {:?}", hint);
        assert!(hint.get("label").is_some(), "hint missing label: {:?}", hint);
        assert!(hint.get("kind").is_some(), "hint missing kind: {:?}", hint);
        // position should have line + character.
        assert!(
            hint["position"].get("line").is_some()
                && hint["position"].get("character").is_some(),
            "hint position missing line/character: {:?}",
            hint
        );
    }
}
