use std::collections::HashMap;
use std::error::Error;

use handl_tooling::{ArgSpec, CommandProvider, CommandSpec, DocumentSymbol, TextContentChange, ToolingHost};
use lsp_server::{Connection, Message, Notification, RequestId, Response};

use handl_expr::{Diagnostic as HandlDiagnostic, DiagnosticLevel};
use handl_platform::PlatformError;
use lsp_types::{
    CompletionItem, Diagnostic as LspDiagnostic, DiagnosticSeverity, DocumentSymbol as LspSymbol,
    Hover, HoverContents, Location as LspLocation, MarkupContent, MarkupKind, Position, Range,
    TextDocumentContentChangeEvent, TextEdit, Uri,
};

/// A minimal LSP state container for the Handl expression language.
pub struct HandlLsp {
    host: ToolingHost,
    documents: HashMap<Uri, String>,
}

impl HandlLsp {
    /// Creates a new LSP instance backed by the supplied tooling host.
    pub fn new(host: ToolingHost) -> Self {
        Self {
            host,
            documents: HashMap::new(),
        }
    }

    /// Formats the entire document and returns a single `TextEdit` that
    /// replaces all of `source`.
    pub fn format(&self, _uri: &Uri, source: &str) -> Result<Vec<TextEdit>, PlatformError> {
        let formatted = self.host.format(source)?;
        let range = full_document_range(source);
        Ok(vec![TextEdit {
            range,
            new_text: formatted,
        }])
    }

    /// Returns a hover containing the formatted expression as markdown.
    pub fn hover(&self, _uri: &Uri, _pos: Position, source: &str) -> Option<Hover> {
        let formatted = self.host.format(source).ok()?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```handl\n{}\n```", formatted),
            }),
            range: None,
        })
    }

    /// Diagnoses `source` and returns LSP diagnostics.
    pub fn diagnostics(&self, _uri: &Uri, source: &str) -> Vec<LspDiagnostic> {
        self.host
            .diagnose(source)
            .into_iter()
            .map(|d: HandlDiagnostic| LspDiagnostic {
                range: d.span.map_or_else(
                    || Range {
                        start: Position::default(),
                        end: Position::default(),
                    },
                    |span| Range {
                        start: offset_to_position(source, span.start),
                        end: offset_to_position(source, span.end),
                    },
                ),
                severity: Some(diagnostic_severity(d.level)),
                code: None,
                code_description: None,
                source: Some("handl-host-lsp".to_string()),
                message: d.message,
                related_information: None,
                tags: None,
                data: None,
            })
            .collect()
    }

    /// Returns completion items for the position in `source`.
    pub fn complete(&self, _uri: &Uri, pos: Position, source: &str) -> Vec<CompletionItem> {
        let offset = position_to_offset(source, pos);
        match self.host.complete(source, offset) {
            Ok(items) => items
                .into_iter()
                .map(|item| CompletionItem {
                    label: item.label,
                    ..Default::default()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Returns the definition location for the identifier at `pos`.
    pub fn goto_def(&self, uri: &Uri, pos: Position, source: &str) -> Option<LspLocation> {
        let offset = position_to_offset(source, pos);
        let loc = self.host.goto_def(source, offset).ok()??;
        Some(LspLocation {
            uri: uri.clone(),
            range: Range {
                start: offset_to_position(source, loc.start),
                end: offset_to_position(source, loc.end),
            },
        })
    }

    /// Returns the top-level document symbols for `source`.
    pub fn document_symbols(&self, _uri: &Uri, source: &str) -> Vec<LspSymbol> {
        match self.host.document_symbols(source) {
            Ok(syms) => syms.into_iter().map(symbol_to_lsp).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Stores an opened document.
    pub fn open(&mut self, uri: Uri, text: String) {
        self.documents.insert(uri, text);
    }

    /// Applies an incremental change to the stored document.
    pub fn change(&mut self, uri: &Uri, change: TextDocumentContentChangeEvent) -> Result<(), PlatformError> {
        let text = self.documents.get(uri).ok_or_else(|| PlatformError::InvalidMetadata(format!("document not open: {uri:?}")))?;
        let new_text = if let Some(range) = change.range {
            let start = position_to_offset(text, range.start);
            let end = position_to_offset(text, range.end);
            self.host.change(text, TextContentChange::replace(start, end, change.text))?
        } else {
            change.text
        };
        self.documents.insert(uri.clone(), new_text);
        Ok(())
    }

    /// Borrows the stored source for a document, if any.
    pub fn get(&self, uri: &Uri) -> Option<&str> {
        self.documents.get(uri).map(|s| s.as_str())
    }
}

fn diagnostic_severity(level: DiagnosticLevel) -> DiagnosticSeverity {
    match level {
        DiagnosticLevel::Error => DiagnosticSeverity::ERROR,
        DiagnosticLevel::Warning => DiagnosticSeverity::WARNING,
        DiagnosticLevel::Info => DiagnosticSeverity::INFORMATION,
    }
}

#[allow(deprecated)]
fn symbol_to_lsp(sym: DocumentSymbol) -> LspSymbol {
    LspSymbol {
        name: sym.name,
        detail: None,
        kind: lsp_types::SymbolKind::VARIABLE,
        tags: None,
        deprecated: None,
        range: Range {
            start: Position::default(),
            end: Position::default(),
        },
        selection_range: Range {
            start: Position {
                line: 0,
                character: sym.start as u32,
            },
            end: Position {
                line: 0,
                character: sym.end as u32,
            },
        },
        children: None,
    }
}

fn full_document_range(source: &str) -> Range {
    if source.is_empty() {
        return Range {
            start: Position::default(),
            end: Position::default(),
        };
    }
    let lines: Vec<&str> = source.split('\n').collect();
    let last_line = (lines.len().saturating_sub(1)) as u32;
    let last_len = lines.last().unwrap_or(&"").len() as u32;
    Range {
        start: Position::default(),
        end: Position {
            line: last_line,
            character: last_len,
        },
    }
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut last_nl = 0usize;
    for (i, c) in source.char_indices() {
        if i > offset {
            break;
        }
        if c == '\n' {
            line = line.saturating_add(1);
            last_nl = i.saturating_add(1);
        }
    }
    Position {
        line,
        character: offset.saturating_sub(last_nl) as u32,
    }
}

fn position_to_offset(source: &str, pos: Position) -> usize {
    let mut offset = 0usize;
    let mut line = 0u32;
    for (i, c) in source.char_indices() {
        if line == pos.line {
            let col = (i - offset) as u32;
            if col >= pos.character {
                return i;
            }
        }
        if c == '\n' {
            line += 1;
            offset = i + 1;
        }
    }
    // Past the requested line/column — clamp to end of the final line.
    source.len()
}

/// Run the Handl LSP server over stdio using the supplied tooling host.
pub fn run_stdio(host: ToolingHost) -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();

    let capabilities = lsp_types::ServerCapabilities {
        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
            lsp_types::TextDocumentSyncKind::INCREMENTAL,
        )),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
        completion_provider: Some(lsp_types::CompletionOptions::default()),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        document_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    };
    let server_capabilities = serde_json::to_value(&capabilities)?;
    let _init = connection.initialize(server_capabilities)?;

    let mut lsp = HandlLsp::new(host);

    for msg in connection.receiver.iter() {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    break;
                }

                match req.method.as_str() {
                    "textDocument/formatting" => {
                        let params: lsp_types::DocumentFormattingParams =
                            serde_json::from_value(req.params)?;
                        let uri = params.text_document.uri;
                        let result: Option<Vec<lsp_types::TextEdit>> =
                            lsp.get(&uri).and_then(|src| lsp.format(&uri, src).ok());
                        respond(&connection, req.id, result)?;
                    }
                    "textDocument/hover" => {
                        let params: lsp_types::HoverParams = serde_json::from_value(req.params)?;
                        let uri = params.text_document_position_params.text_document.uri;
                        let pos = params.text_document_position_params.position;
                        let hover = lsp.get(&uri).and_then(|src| lsp.hover(&uri, pos, src));
                        respond(&connection, req.id, hover)?;
                    }
                    "textDocument/completion" => {
                        let params: lsp_types::CompletionParams =
                            serde_json::from_value(req.params)?;
                        let uri = params.text_document_position.text_document.uri;
                        let pos = params.text_document_position.position;
                        let items = lsp
                            .get(&uri)
                            .map(|src| lsp.complete(&uri, pos, src))
                            .unwrap_or_default();
                        respond(&connection, req.id, Some(items))?;
                    }
                    "textDocument/definition" => {
                        let params: lsp_types::GotoDefinitionParams =
                            serde_json::from_value(req.params)?;
                        let uri = params.text_document_position_params.text_document.uri;
                        let pos = params.text_document_position_params.position;
                        let loc = lsp.get(&uri).and_then(|src| lsp.goto_def(&uri, pos, src));
                        respond(&connection, req.id, loc)?;
                    }
                    "textDocument/documentSymbol" => {
                        let params: lsp_types::DocumentSymbolParams =
                            serde_json::from_value(req.params)?;
                        let uri = params.text_document.uri;
                        let syms = lsp
                            .get(&uri)
                            .map(|src| lsp.document_symbols(&uri, src))
                            .unwrap_or_default();
                        respond(&connection, req.id, syms)?;
                    }
                    _ => {}
                }
            }
            Message::Notification(not) => match not.method.as_str() {
                "textDocument/didOpen" => {
                    let params: lsp_types::DidOpenTextDocumentParams =
                        serde_json::from_value(not.params)?;
                    let uri = params.text_document.uri;
                    lsp.open(uri.clone(), params.text_document.text);
                    publish_diagnostics(&connection, &lsp, &uri)?;
                }
                "textDocument/didChange" => {
                    let params: lsp_types::DidChangeTextDocumentParams =
                        serde_json::from_value(not.params)?;
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
    id: RequestId,
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
        let params = lsp_types::PublishDiagnosticsParams {
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

/// Command provider that registers the `lsp` CLI command.
pub struct LspCommandProvider;

impl LspCommandProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LspCommandProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandProvider for LspCommandProvider {
    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec::new("lsp", "Start the stdio LSP server")
            .arg(ArgSpec::new("args").description("LSP server arguments").multiple())]
    }

    fn run(
        &self,
        _command: &str,
        _args: &[String],
        _host: &ToolingHost,
    ) -> Result<serde_json::Value, PlatformError> {
        Ok(serde_json::Value::Null)
    }

    fn run_owned(
        &self,
        _command: &str,
        _args: Vec<String>,
        host: ToolingHost,
    ) -> Result<serde_json::Value, PlatformError> {
        run_stdio(host).map_err(|e| PlatformError::Plugin(e.to_string()))?;
        Ok(serde_json::Value::Null)
    }
}
