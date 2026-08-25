use handl_kit_canonical::CanonicalKit;
use handl_host_lsp::HandlLsp;
use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_fmt_expression::ExpressionFormatter;
use handl_plugin_lsp_completion::BasicCompletion;
use handl_plugin_lsp_goto::BasicGotoDef;
use handl_plugin_lsp_symbols::BasicDocumentSymbols;
use handl_tooling::ToolingHost;
use lsp_types::{Position, Range, TextDocumentContentChangeEvent, Uri};

fn host() -> ToolingHost {
    ToolingHost::new(
        CanonicalKit::new(),
        Box::new(ExpressionFormatter::new()),
        Box::new(ExpressionDiagnostics::new()),
    )
    .with_completion_plugin(Box::new(BasicCompletion::new()))
    .with_goto_def_plugin(Box::new(BasicGotoDef::new()))
    .with_document_symbols_plugin(Box::new(BasicDocumentSymbols::new()))
}

#[test]
fn formats_pipe() {
    let lsp = HandlLsp::new(host());
    let uri: Uri = "file:///test.handl".parse().unwrap();
    let edits = lsp.format(&uri, "1 |> add").unwrap();
    assert!(!edits.is_empty());
    assert_eq!(edits[0].new_text, "1 |> add");
}

#[test]
fn diagnoses_double_negation() {
    let lsp = HandlLsp::new(host());
    let uri: Uri = "file:///test.handl".parse().unwrap();
    let diags = lsp.diagnostics(&uri, "not not true");
    assert!(!diags.is_empty());
}

#[test]
fn completion_returns_binding() {
    let lsp = HandlLsp::new(host());
    let uri: Uri = "file:///test.handl".parse().unwrap();
    let src = "let a = 1; a";
    let items = lsp.complete(&uri, Position { line: 0, character: 12 }, src);
    assert!(items.iter().any(|i| i.label == "a"));
}

#[test]
fn goto_def_returns_location() {
    let lsp = HandlLsp::new(host());
    let uri: Uri = "file:///test.handl".parse().unwrap();
    let src = "let a = 1; a";
    let loc = lsp
        .goto_def(&uri, Position { line: 0, character: 11 }, src)
        .expect("goto_def should return a location");
    assert_eq!(loc.range.start.character, 4);
    assert_eq!(loc.range.end.character, 5);
    assert_eq!(loc.uri, uri);
}

#[test]
fn document_symbols_returns_top_level() {
    let lsp = HandlLsp::new(host());
    let uri: Uri = "file:///test.handl".parse().unwrap();
    let src = "let a = 1; const b = 2;";
    let syms = lsp.document_symbols(&uri, src);
    assert_eq!(syms.len(), 2);
    assert_eq!(syms[0].name, "a");
    assert_eq!(syms[1].name, "b");
}

#[test]
fn incremental_change_updates_document() {
    let mut lsp = HandlLsp::new(host());
    let uri: Uri = "file:///test.handl".parse().unwrap();
    lsp.open(uri.clone(), "let a = 1; a".to_string());
    let change = TextDocumentContentChangeEvent {
        range: Some(Range {
            start: Position { line: 0, character: 12 },
            end: Position { line: 0, character: 12 },
        }),
        range_length: None,
        text: " + 2".to_string(),
    };
    lsp.change(&uri, change).unwrap();
    assert_eq!(lsp.get(&uri).unwrap(), "let a = 1; a + 2");
}
