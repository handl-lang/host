use handl_host_tooling::lsp::HoverRequest;
use handl_kit_data_canonical::language_from_yaml;
use handl_plugin_lsp::LspEngine;

#[test]
fn hover_assign_returns_signature_and_trailing_docs() {
    let lang = language_from_yaml().expect("canonical language assembles");
    let engine = LspEngine::new(&lang);

    let source = "square :: 1 -- a value";
    let pos = source.find("square").unwrap() + 1;
    let request = HoverRequest {
        source: source.to_string(),
        pos,
        path: "test.hnl".to_string(),
    };

    let response = engine.hover(&request);
    let md = response.0.expect("expected hover markdown");
    assert!(md.contains("square :: 1"), "expected signature in hover: {}", md);
    assert!(md.contains("a value"), "expected trailing docs in hover: {}", md);
}
