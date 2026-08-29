use handl_kit_canonical::CanonicalKit;
use handl_plugin_fmt_expression::ExpressionFormatter;
use handl_plugin_lsp_completion::BasicCompletion;
use handl_plugin_lsp_goto::BasicGotoDef;
use handl_plugin_lsp_symbol::BasicDocumentSymbols;
use handl_host_tooling::{Location, TextContentChange, ToolingHost};

fn host() -> ToolingHost<CanonicalKit> {
    ToolingHost::new(CanonicalKit::new(), Box::new(ExpressionFormatter::new()))
        .with_completion_plugin(Box::new(BasicCompletion::new()))
        .with_goto_def_plugin(Box::new(BasicGotoDef::new()))
        .with_document_symbols_plugin(Box::new(BasicDocumentSymbols::new()))
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn complete_includes_let_binding() {
    let h = host();
    let items = h.complete("let a = 1; a", 12).unwrap();
    assert!(items.iter().any(|i| i.label == "a"), "expected binding 'a' in {items:?}");
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn complete_offers_keyword() {
    let h = host();
    let items = h.complete("", 0).unwrap();
    assert!(items.iter().any(|i| i.label == "let"));
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn goto_def_finds_let_binding() {
    let h = host();
    let src = "let a = 1; a";
    let loc = h.goto_def(src, 11).unwrap().expect("goto_def should return a location");
    assert_eq!(loc, Location::new(4, 5));
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn document_symbols_for_let_and_const() {
    let h = host();
    let src = "let a = 1; const b = 2;";
    let syms = h.document_symbols(src).unwrap();
    assert_eq!(syms.len(), 2);
    assert_eq!(syms[0].name, "a");
    assert_eq!(syms[1].name, "b");
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn incremental_change_appends_text() {
    let mut h = host();
    let src = "let a = 1; a";
    let new = h.change(src, TextContentChange::insert(12, " + 2")).unwrap();
    assert_eq!(new, "let a = 1; a + 2");
}

#[test]
#[ignore = "LSP plugins are stubbed"]
fn format_still_works() {
    let h = host();
    let out = h.format("let a = 1").unwrap();
    assert_eq!(out, "let a = 1;");
}
