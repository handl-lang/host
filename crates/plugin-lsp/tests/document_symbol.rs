use handl_host_tooling::lsp::DocumentSymbolsRequest;
use handl_kit_data_canonical::language_from_yaml;
use handl_plugin_lsp::LspEngine;

#[test]
fn document_symbol_lists_law_and_assign() {
    let lang = language_from_yaml().expect("canonical language assembles");
    let engine = LspEngine::new(&lang);

    let source = "law add : 1 => 2\nsquare :: 1";
    let request = DocumentSymbolsRequest(source.to_string());
    let response = engine.document_symbols(&request);

    let names: Vec<_> = response.0.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.iter().any(|n| *n == "add"),
        "expected 'add' in {:?}",
        names
    );
    assert!(
        names.iter().any(|n| *n == "square"),
        "expected 'square' in {:?}",
        names
    );
}
