//! Provider trait and canonical registry for the generic `LspEngine`.
//!
//! Each provider is keyed only by the `rule.kind` string declared in `lsp.yaml`.
//! Implementations delegate to the helper functions in `crate::providers`; this
//! module contains no handl-specific surface syntax beyond the rule-kind keys
//! registered in `default_providers`.

use crate::providers::{
    ExportMatch, ParsedModule, run_export, run_exported_names, run_import_hover, run_import_name,
    run_import_path, run_folding_range, run_signature_help, run_symbol,
};
use handl_host_tooling::lsp::{
    CompletionItem, CompletionRequest, DocumentSymbol, DocumentSymbolsRequest, FoldingRange,
    FoldingRangeRequest, GotoDefRequest, HoverRequest, SignatureHelpRequest, SignatureHelpResponse,
};
use handl_language::{Language, LspDef, LspRule, Span};
use std::collections::HashMap;

/// Trait implemented by every `rule.kind` provider.
///
/// The default implementations return empty / `None`, so a provider only needs
/// to override the request methods it actually handles.
pub(crate) trait LspProvider: Send + Sync {
    fn completion(
        &self,
        _lang: &Language,
        _def: &LspDef,
        _module: &ParsedModule,
        _rule: &LspRule,
        _request: &CompletionRequest,
    ) -> Vec<CompletionItem> {
        Vec::new()
    }

    fn hover(
        &self,
        _lang: &Language,
        _def: &LspDef,
        _module: &ParsedModule,
        _rule: &LspRule,
        _request: &HoverRequest,
    ) -> Option<String> {
        None
    }

    fn goto(
        &self,
        _lang: &Language,
        _def: &LspDef,
        _module: &ParsedModule,
        _rule: &LspRule,
        _request: &GotoDefRequest,
    ) -> Option<Span> {
        None
    }

    fn document_symbols(
        &self,
        _lang: &Language,
        _def: &LspDef,
        _module: &ParsedModule,
        _rule: &LspRule,
        _request: &DocumentSymbolsRequest,
    ) -> Vec<DocumentSymbol> {
        Vec::new()
    }

    fn signature_help(
        &self,
        _lang: &Language,
        _def: &LspDef,
        _module: &ParsedModule,
        _rule: &LspRule,
        _request: &SignatureHelpRequest,
    ) -> Option<SignatureHelpResponse> {
        None
    }

    fn folding_range(
        &self,
        _lang: &Language,
        _def: &LspDef,
        _module: &ParsedModule,
        _rule: &LspRule,
        _request: &FoldingRangeRequest,
    ) -> Vec<FoldingRange> {
        Vec::new()
    }
}

struct ImportPathProvider;
struct ImportNameProvider;
struct ExportNameProvider;
struct ExportProvider;
struct ImportHoverProvider;
struct SymbolProvider;

impl LspProvider for ImportPathProvider {
    fn completion(
        &self,
        lang: &Language,
        _def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        request: &CompletionRequest,
    ) -> Vec<CompletionItem> {
        run_import_path(lang, module, request.pos, &request.path, rule)
    }
}

impl LspProvider for ImportNameProvider {
    fn completion(
        &self,
        lang: &Language,
        def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        request: &CompletionRequest,
    ) -> Vec<CompletionItem> {
        run_import_name(lang, module, def, request.pos, &request.path, rule)
    }
}

impl LspProvider for ExportNameProvider {
    fn completion(
        &self,
        lang: &Language,
        def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        _request: &CompletionRequest,
    ) -> Vec<CompletionItem> {
        run_exported_names(lang, module, def, rule)
    }
}

impl LspProvider for ExportProvider {
    fn hover(
        &self,
        lang: &Language,
        _def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        request: &HoverRequest,
    ) -> Option<String> {
        let m = run_export(lang, module, request.pos, rule)?;
        format_export_hover(&m)
    }

    fn goto(
        &self,
        lang: &Language,
        _def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        request: &GotoDefRequest,
    ) -> Option<Span> {
        run_export(lang, module, request.pos, rule).map(|m| m.name_span)
    }
}

impl LspProvider for ImportHoverProvider {
    fn hover(
        &self,
        lang: &Language,
        _def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        request: &HoverRequest,
    ) -> Option<String> {
        run_import_hover(lang, module, request.pos, &request.path, rule)
    }
}

impl LspProvider for SymbolProvider {
    fn document_symbols(
        &self,
        lang: &Language,
        _def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        _request: &DocumentSymbolsRequest,
    ) -> Vec<DocumentSymbol> {
        run_symbol(lang, module, rule)
    }
}

struct SignatureHelpProvider;

impl LspProvider for SignatureHelpProvider {
    fn signature_help(
        &self,
        lang: &Language,
        _def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        request: &SignatureHelpRequest,
    ) -> Option<SignatureHelpResponse> {
        run_signature_help(lang, module, request.pos, rule)
    }
}

struct FoldingRangeProvider;

impl LspProvider for FoldingRangeProvider {
    fn folding_range(
        &self,
        lang: &Language,
        _def: &LspDef,
        module: &ParsedModule,
        rule: &LspRule,
        _request: &FoldingRangeRequest,
    ) -> Vec<FoldingRange> {
        run_folding_range(lang, module, rule)
    }
}

/// Format an `ExportMatch` into the markdown shown on hover.
fn format_export_hover(m: &ExportMatch) -> Option<String> {
    let mut md = String::new();
    if let Some(docs) = &m.docs {
        md.push_str(docs);
    }
    if let Some(signature) = &m.signature {
        if !md.is_empty() {
            md.push_str("\n\n");
        }
        md.push_str("```\n");
        md.push_str(signature);
        md.push('\n');
        md.push_str("```");
    }
    if md.is_empty() { None } else { Some(md) }
}

/// Build the canonical provider registry.
pub(crate) fn default_providers() -> HashMap<String, Box<dyn LspProvider + Send + Sync>> {
    let mut providers: HashMap<String, Box<dyn LspProvider + Send + Sync>> = HashMap::new();
    providers.insert("import_path".into(), Box::new(ImportPathProvider));
    providers.insert("import_name".into(), Box::new(ImportNameProvider));
    providers.insert("export_name".into(), Box::new(ExportNameProvider));
    providers.insert("export".into(), Box::new(ExportProvider));
    providers.insert("import_hover".into(), Box::new(ImportHoverProvider));
    providers.insert("symbol".into(), Box::new(SymbolProvider));
    providers.insert("signature".into(), Box::new(SignatureHelpProvider));
    providers.insert("folding_range".into(), Box::new(FoldingRangeProvider));
    providers
}
