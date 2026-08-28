use handl_kit_canonical::CanonicalKit;
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_tooling::lsp::{
    DocumentSymbol, DocumentSymbolsPlugin, DocumentSymbolsRequest, DocumentSymbolsResponse,
};

/// A minimal document-symbols plugin that produces one symbol per top-level
/// declaration using token spans.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicDocumentSymbols;

impl BasicDocumentSymbols {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentSymbolsPlugin for BasicDocumentSymbols {
    fn run(&self, request: &DocumentSymbolsRequest) -> Result<DocumentSymbolsResponse, PlatformError> {
        let kit = CanonicalKit::new();
        let tokens = kit.lex(&request.0);

        let mut symbols = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let (name, span) = match &tokens[i].kind {
                CanonicalKind::Identifier(kw) if kw == "let" || kw == "const" => {
                    if let Some(t) = tokens.get(i + 1) {
                        if let CanonicalKind::Identifier(name) = &t.kind {
                            let span = t.span;
                            i += 1;
                            (name.clone(), span)
                        } else {
                            i += 1;
                            continue;
                        }
                    } else {
                        break;
                    }
                }
                CanonicalKind::Identifier(kw) if kw == "module" => {
                    if let Some(t) = tokens.get(i + 1) {
                        if let CanonicalKind::Identifier(name) = &t.kind {
                            let span = t.span;
                            i += 1;
                            (name.clone(), span)
                        } else {
                            i += 1;
                            continue;
                        }
                    } else {
                        break;
                    }
                }
                CanonicalKind::Identifier(kw) if kw == "import" || kw == "export" => {
                    if let Some(t) = tokens.get(i + 1) {
                        if let CanonicalKind::Identifier(name) = &t.kind {
                            let span = t.span;
                            i += 1;
                            (name.clone(), span)
                        } else {
                            i += 1;
                            continue;
                        }
                    } else {
                        break;
                    }
                }
                _ => {
                    i += 1;
                    continue;
                }
            };

            symbols.push(DocumentSymbol::new(name, span.start, span.end));
            i += 1;
        }

        Ok(DocumentSymbolsResponse(symbols))
    }
}
