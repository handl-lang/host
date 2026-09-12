//! Generic, data-driven LSP engine (ADR-0100).
//!
//! `plugin-lsp` consumes `Language::lsp` and dispatches LSP requests by the
//! method-keyed rule map.  Each rule's `kind` selects a provider; provider
//! configuration lives in `LspRule::config` as free-form data.

use handl_host_tooling::lsp::{
    CompletionRequest, CompletionResponse, DocumentSymbolsRequest, DocumentSymbolsResponse,
    GotoDefRequest, GotoDefResponse, HoverRequest, HoverResponse, Location,
};
use handl_language::{Language, LspDef};

mod providers;
pub mod validate;

pub use validate::validate_lsp;

/// Generic LSP engine bound to a `Language`.
pub struct LspEngine<'l> {
    lang: &'l Language,
    /// The first `LspDef` in `Language::lsp` (multiple fragments can be merged;
    /// for now the engine consults the canonical one with id `canonical.lsp`).
    def: Option<&'l LspDef>,
}

impl<'l> LspEngine<'l> {
    /// Create an engine for `lang`.
    pub fn new(lang: &'l Language) -> Self {
        let def = lang
            .lsp
            .get_by_id("canonical.lsp")
            .or_else(|| lang.lsp.iter().next());
        Self { lang, def }
    }

    /// Evaluate `textDocument/completion` against the language's rules.
    pub fn completion(&self, request: &CompletionRequest) -> CompletionResponse {
        let Some(def) = self.def else {
            return CompletionResponse(Vec::new());
        };
        let Some(rule_ids) = def.lsp.get("textDocument/completion") else {
            return CompletionResponse(Vec::new());
        };

        let module = match providers::parse_module(self.lang, &request.source) {
            Ok(m) => m,
            Err(_) => return CompletionResponse(Vec::new()),
        };

        let mut out = Vec::new();
        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            let items = match rule.kind.as_str() {
                "import_path" => providers::run_import_path(self.lang, &module, request.pos, &request.path, rule),
                "import_name" => providers::run_import_name(self.lang, &module, def, request.pos, &request.path, rule),
                "export_name" => providers::run_exported_names(self.lang, &module, def, rule),
                _ => Vec::new(),
            };
            out.extend(items);
        }

        CompletionResponse(out)
    }

    /// Evaluate `textDocument/hover` against the language's rules.
    pub fn hover(&self, request: &HoverRequest) -> HoverResponse {
        let Some(def) = self.def else {
            return HoverResponse(None);
        };
        let Some(rule_ids) = def.lsp.get("textDocument/hover") else {
            return HoverResponse(None);
        };

        // Parse once, then evaluate the export rules in declaration order.
        let module = match providers::parse_module(self.lang, &request.source) {
            Ok(m) => m,
            Err(_) => return HoverResponse(None),
        };

        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            if rule.kind == "export" {
                if let Some(m) = providers::run_export(self.lang, &module, request.pos, rule) {
                    let mut md = String::new();
                    if let Some(docs) = m.docs {
                        md.push_str(&docs);
                    }
                    if let Some(signature) = m.signature {
                        if !md.is_empty() {
                            md.push_str("\n\n");
                        }
                        md.push_str("```\n");
                        md.push_str(&signature);
                        md.push('\n');
                        md.push_str("```");
                    }
                    if md.is_empty() {
                        return HoverResponse(None);
                    }
                    return HoverResponse(Some(md));
                }
            } else if rule.kind == "import_hover" {
                if let Some(md) =
                    providers::run_import_hover(self.lang, &module, request.pos, &request.path, rule)
                {
                    return HoverResponse(Some(md));
                }
            }
        }

        HoverResponse(None)
    }

    /// Evaluate `textDocument/definition` against the language's rules.
    pub fn goto(&self, request: &GotoDefRequest) -> GotoDefResponse {
        let Some(def) = self.def else {
            return GotoDefResponse(None);
        };
        let Some(rule_ids) = def.lsp.get("textDocument/definition") else {
            return GotoDefResponse(None);
        };

        let module = match providers::parse_module(self.lang, &request.source) {
            Ok(m) => m,
            Err(_) => return GotoDefResponse(None),
        };

        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            if rule.kind != "export" {
                continue;
            }
            if let Some(m) = providers::run_export(self.lang, &module, request.pos, rule) {
                return GotoDefResponse(Some(Location::new(m.name_span.start, m.name_span.end)));
            }
        }

        GotoDefResponse(None)
    }

    /// Evaluate `textDocument/documentSymbol` against the language's rules.
    pub fn document_symbols(&self, request: &DocumentSymbolsRequest) -> DocumentSymbolsResponse {
        let Some(def) = self.def else {
            return DocumentSymbolsResponse(Vec::new());
        };
        let Some(rule_ids) = def.lsp.get("textDocument/documentSymbol") else {
            return DocumentSymbolsResponse(Vec::new());
        };

        let module = match providers::parse_module(self.lang, &request.0) {
            Ok(m) => m,
            Err(_) => return DocumentSymbolsResponse(Vec::new()),
        };

        let mut out = Vec::new();
        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            if rule.kind == "symbol" {
                out.extend(providers::run_symbol(self.lang, &module, rule));
            }
        }

        DocumentSymbolsResponse(out)
    }
}
