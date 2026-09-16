use std::io::Write;

use crate::protocol::send_response;
use handl_host_tooling::SemanticTokenLegend;
use serde_json::{Value, json};
pub fn handle_initialize(writer: &mut impl Write, id: Value, legend: &SemanticTokenLegend) {
    let result = build_initialize_result(legend);
    send_response(writer, id, result);
}

pub fn build_initialize_result(legend: &SemanticTokenLegend) -> Value {
    json!({
        "capabilities": {
            "semanticTokensProvider": {
                "legend": {
                    "tokenTypes": legend.token_types.clone(),
                    "tokenModifiers": legend.token_modifiers.clone(),
                },
                "full": true,
                "range": true,
            },
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": { "includeText": true }
            },
            "hoverProvider": true,
            "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
            "definitionProvider": true,
            "referencesProvider": true,
            "renameProvider": { "prepareProvider": true },
            "codeLensProvider": true,
            "documentSymbolProvider": true,
            "documentFormattingProvider": true,
            "documentRangeFormattingProvider": true,
            "documentOnTypeFormattingProvider": {
                "firstTriggerCharacter": "\n",
                "moreTriggerCharacter": ["}", ")", "]"]
            },
            "foldingRangeProvider": true,
            "documentHighlightProvider": true,
            "selectionRangeProvider": true,
            "workspaceSymbolProvider": true,
            "codeActionProvider": true,
            "typeDefinitionProvider": true,
            "implementationProvider": true,
            "linkedEditingRangeProvider": true,
            "monikerProvider": true,
            "documentLinkProvider": true,
            "callHierarchyProvider": true,
            "typeHierarchyProvider": true,
            "colorProvider": true,
            "completionProvider": {
                "triggerCharacters": [".", ">", "|", "-", " "],
                "resolveProvider": true
            },
            "inlayHintProvider": { "resolveProvider": true },
            "executeCommandProvider": {
                "commands": [
                    "handl.showRefinement",
                    "handl.runArrow",
                    "handl.formatFile",
                    "handl.showContract"
                ]
            },
        },
        "serverInfo": {
            "name": "handl",
            "version": "0.1.0",
        }
    })
}
