//! Generic, data-driven LSP engine (ADR-0100).
//!
//! `plugin-lsp` consumes `Language::lsp` and dispatches LSP requests by the
//! method-keyed rule map.  Each rule's `kind` selects a provider via a
//! language-agnostic registry; provider configuration lives in `LspRule::config`
//! as free-form data.

use handl_host_tooling::lsp::{
    CompletionRequest, CompletionResponse, DocumentSymbolsRequest, DocumentSymbolsResponse,
    FoldingRange, FoldingRangeRequest, FoldingRangeResponse, GotoDefRequest, GotoDefResponse,
    HoverRequest, HoverResponse, Location, SignatureHelpRequest, SignatureHelpResponse,
};
use handl_language::{Language, LspDef};
use std::collections::HashMap;
use std::sync::Mutex;

mod lsp_provider;
mod providers;
pub mod validate;

use crate::lsp_provider::{LspProvider, default_providers};
use crate::providers::ParsedModule;
pub use validate::validate_lsp;

/// Generic LSP engine bound to a `Language`.
pub struct LspEngine<'l> {
    lang: &'l Language,
    /// The first `LspDef` in `Language::lsp` (multiple fragments can be merged;
    /// for now the engine consults the canonical one with id `canonical.lsp`).
    def: Option<&'l LspDef>,
    /// Registry mapping `rule.kind` strings to their provider implementations.
    providers: HashMap<String, Box<dyn LspProvider + Send + Sync>>,
    /// Source-level parse cache, keyed by the full source string.
    cache: Mutex<HashMap<String, ParsedModule>>,
}

impl<'l> LspEngine<'l> {
    /// Create an engine for `lang`.
    pub fn new(lang: &'l Language) -> Self {
        let def = lang
            .lsp
            .get_by_id("canonical.lsp")
            .or_else(|| lang.lsp.iter().next());
        Self {
            lang,
            def,
            providers: default_providers(),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Return a cached parse of `src`, computing and storing it if necessary.
    fn module_for(&self, src: &str) -> ParsedModule {
        let mut guard = self.cache.lock().unwrap();
        if let Some(module) = guard.get(src) {
            return module.clone();
        }
        let module =
            providers::parse_module(self.lang, src.to_string()).unwrap_or_else(|_| ParsedModule {
                src: src.to_string(),
                tokens: Vec::new(),
                statements: Vec::new(),
            });
        guard.insert(src.to_string(), module.clone());
        module
    }

    /// Evaluate `textDocument/completion` against the language's rules.
    pub fn completion(&self, request: &CompletionRequest) -> CompletionResponse {
        let Some(def) = self.def else {
            return CompletionResponse(Vec::new());
        };
        let Some(rule_ids) = def.lsp.get("textDocument/completion") else {
            return CompletionResponse(Vec::new());
        };

        let module = self.module_for(&request.source);

        let mut out = Vec::new();
        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            let Some(provider) = self.providers.get(&rule.kind) else {
                continue;
            };
            out.extend(provider.completion(self.lang, def, &module, rule, request));
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

        let module = self.module_for(&request.source);

        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            let Some(provider) = self.providers.get(&rule.kind) else {
                continue;
            };
            if let Some(md) = provider.hover(self.lang, def, &module, rule, request) {
                return HoverResponse(Some(md));
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

        let module = self.module_for(&request.source);

        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            let Some(provider) = self.providers.get(&rule.kind) else {
                continue;
            };
            if let Some(span) = provider.goto(self.lang, def, &module, rule, request) {
                return GotoDefResponse(Some(Location::new(span.start, span.end)));
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

        let module = self.module_for(&request.0);

        let mut out = Vec::new();
        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            let Some(provider) = self.providers.get(&rule.kind) else {
                continue;
            };
            out.extend(provider.document_symbols(self.lang, def, &module, rule, request));
        }

        DocumentSymbolsResponse(out)
    }

    /// Evaluate `textDocument/signatureHelp` against the language's rules.
    pub fn signature_help(&self, request: &SignatureHelpRequest) -> SignatureHelpResponse {
        let Some(def) = self.def else {
            return SignatureHelpResponse::empty();
        };
        let Some(rule_ids) = def.lsp.get("textDocument/signatureHelp") else {
            return SignatureHelpResponse::empty();
        };

        let module = self.module_for(&request.source);

        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            let Some(provider) = self.providers.get(&rule.kind) else {
                continue;
            };
            if let Some(resp) = provider.signature_help(self.lang, def, &module, rule, request) {
                return resp;
            }
        }

        SignatureHelpResponse::empty()
    }

    /// Evaluate `textDocument/foldingRange` against the language's rules.
    pub fn folding_range(&self, request: &FoldingRangeRequest) -> FoldingRangeResponse {
        let Some(def) = self.def else {
            return FoldingRangeResponse(Vec::new());
        };
        let Some(rule_ids) = def.lsp.get("textDocument/foldingRange") else {
            return FoldingRangeResponse(Vec::new());
        };

        let module = self.module_for(&request.source);

        let mut out: Vec<FoldingRange> = Vec::new();
        for rule_id in rule_ids {
            let Some(rule) = def.rules.get(rule_id) else {
                continue;
            };
            let Some(provider) = self.providers.get(&rule.kind) else {
                continue;
            };
            out.extend(provider.folding_range(self.lang, def, &module, rule, request));
        }

        FoldingRangeResponse(out)
    }
}
