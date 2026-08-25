use std::error::Error;

use handl_kit_canonical::CanonicalKit;
use handl_host_lsp::HandlLsp;
use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_fmt_expression::ExpressionFormatter;
use handl_plugin_lsp_completion::BasicCompletion;
use handl_plugin_lsp_goto::BasicGotoDef;
use handl_plugin_lsp_symbols::BasicDocumentSymbols;
use handl_tooling::ToolingHost;
use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    CompletionParams, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DocumentFormattingParams, DocumentSymbolParams, GotoDefinitionParams, HoverParams,
    HoverProviderCapability, OneOf, PublishDiagnosticsParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();

    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::INCREMENTAL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        completion_provider: Some(lsp_types::CompletionOptions::default()),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        ..Default::default()
    };
    let server_capabilities = serde_json::to_value(&capabilities)?;
    let _init = connection.initialize(server_capabilities)?;

    let host = ToolingHost::new(
        CanonicalKit::new(),
        Box::new(ExpressionFormatter::new()),
        Box::new(ExpressionDiagnostics::new()),
    )
    .with_completion_plugin(Box::new(BasicCompletion::new()))
    .with_goto_def_plugin(Box::new(BasicGotoDef::new()))
    .with_document_symbols_plugin(Box::new(BasicDocumentSymbols::new()));

    let mut lsp = HandlLsp::new(host);

    for msg in connection.receiver.iter() {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    break;
                }

                match req.method.as_str() {
                    "textDocument/formatting" => {
                        let params: DocumentFormattingParams = serde_json::from_value(req.params)?;
                        let uri = params.text_document.uri;
                        let result: Option<Vec<lsp_types::TextEdit>> =
                            lsp.get(&uri).and_then(|src| lsp.format(&uri, src).ok());
                        respond(&connection, req.id, result)?;
                    }
                    "textDocument/hover" => {
                        let params: HoverParams = serde_json::from_value(req.params)?;
                        let uri = params.text_document_position_params.text_document.uri;
                        let pos = params.text_document_position_params.position;
                        let hover = lsp.get(&uri).and_then(|src| lsp.hover(&uri, pos, src));
                        respond(&connection, req.id, hover)?;
                    }
                    "textDocument/completion" => {
                        let params: CompletionParams = serde_json::from_value(req.params)?;
                        let uri = params.text_document_position.text_document.uri;
                        let pos = params.text_document_position.position;
                        let items = lsp.get(&uri).map(|src| lsp.complete(&uri, pos, src)).unwrap_or_default();
                        respond(&connection, req.id, Some(items))?;
                    }
                    "textDocument/definition" => {
                        let params: GotoDefinitionParams = serde_json::from_value(req.params)?;
                        let uri = params.text_document_position_params.text_document.uri;
                        let pos = params.text_document_position_params.position;
                        let loc = lsp.get(&uri).and_then(|src| lsp.goto_def(&uri, pos, src));
                        respond(&connection, req.id, loc)?;
                    }
                    "textDocument/documentSymbol" => {
                        let params: DocumentSymbolParams = serde_json::from_value(req.params)?;
                        let uri = params.text_document.uri;
                        let syms = lsp.get(&uri).map(|src| lsp.document_symbols(&uri, src)).unwrap_or_default();
                        respond(&connection, req.id, syms)?;
                    }
                    _ => {}
                }
            }
            Message::Notification(not) => match not.method.as_str() {
                "textDocument/didOpen" => {
                    let params: DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
                    let uri = params.text_document.uri;
                    lsp.open(uri.clone(), params.text_document.text);
                    publish_diagnostics(&connection, &lsp, &uri)?;
                }
                "textDocument/didChange" => {
                    let params: DidChangeTextDocumentParams = serde_json::from_value(not.params)?;
                    let uri = params.text_document.uri;
                    for change in params.content_changes {
                        lsp.change(&uri, change)?;
                    }
                    publish_diagnostics(&connection, &lsp, &uri)?;
                }
                _ => {}
            },
            Message::Response(_) => {}
        }
    }

    io_threads.join()?;
    Ok(())
}

fn respond<T: serde::Serialize>(
    connection: &Connection,
    id: lsp_server::RequestId,
    result: T,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let value = serde_json::to_value(result)?;
    connection.sender.send(Message::Response(Response {
        id,
        result: Some(value),
        error: None,
    }))?;
    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    lsp: &HandlLsp,
    uri: &lsp_types::Uri,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    if let Some(source) = lsp.get(uri) {
        let diagnostics = lsp.diagnostics(uri, source);
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: None,
        };
        connection.sender.send(Message::Notification(Notification {
            method: "textDocument/publishDiagnostics".to_string(),
            params: serde_json::to_value(params)?,
        }))?;
    }
    Ok(())
}
