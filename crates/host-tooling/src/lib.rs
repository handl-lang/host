//! Tooling / LSP host: the plugin-host contract for tooling capabilities.
//!
//! The formatting and linting *implementations* live in `plugin-ext-fmt` and
//! `plugin-ext-lint`; this crate owns the host surface (plugin slots +
//! delegates) and the LSP support types (`lsp`, `semantic_tokens`,
//! `grammar`).

use handl_language::Language;
use handl_platform::PlatformError;
use handl_plugin_ext_lint::LintContext;

/// A diagnostic from the shared `plugin-ext-diag` model (ADR-0091).
///
/// Re-exported so `ToolingHost::diagnose` keeps its name and shape while the
/// crate no longer carries a private copy of the type: `plugin-ext-diag`'s
/// `Diagnostic` also carries the rule name, line/column spans and an optional
/// machine-applicable fix, which the former local struct dropped.
pub use handl_plugin_ext_diag::{Diagnostic, Severity};

/// Builds a single error-level diagnostic carrying `message`.
///
/// `ToolingHost::diagnose` returns diagnostics rather than a `Result` (its
/// contract predates the shared model), so failures are surfaced in-band.
fn error_diagnostic(message: &str) -> Diagnostic {
    Diagnostic::new(
        "host-tooling.error",
        Severity::Error,
        handl_plugin_ext_diag::Span::default(),
        message,
    )
}

pub mod command;

pub mod config;
pub use config::*;

pub mod lsp;
pub use lsp::*;

pub mod grammar;
pub use grammar::*;
#[cfg(test)]
mod grammar_tests;

pub mod semantic_tokens;
pub use semantic_tokens::*;
#[cfg(test)]
mod semantic_tokens_tests;

/// Tooling / LSP host that decouples formatter and diagnostics from parser internals.
pub struct ToolingHost {
    completion: Option<Box<dyn CompletionPlugin>>,
    goto_def: Option<Box<dyn GotoDefPlugin>>,
    document_symbols: Option<Box<dyn DocumentSymbolsPlugin>>,
    incremental_sync: Option<Box<dyn IncrementalSyncPlugin>>,
    semantic_tokens: Option<Box<dyn SemanticTokensPlugin>>,
    hover: Option<Box<dyn HoverPlugin>>,
    signature_help: Option<Box<dyn SignatureHelpPlugin>>,
    folding_range: Option<Box<dyn FoldingRangePlugin>>,
    /// The injected lint bundle [`diagnose`](Self::diagnose) lints through —
    /// the language plus the `.hnl`-rule resolver (ADR-0128 §"The 0.8 seam").
    lint: Option<LintContext>,
}

impl ToolingHost {
    /// Creates a new tooling host.
    pub fn new() -> Self {
        Self {
            completion: None,
            goto_def: None,
            document_symbols: None,
            incremental_sync: Some(Box::new(BasicIncrementalSync::new())),
            semantic_tokens: None,
            hover: None,
            signature_help: None,
            folding_range: None,
            lint: None,
        }
    }

    /// Injects the [`LintContext`] `diagnose` lints through — the host
    /// supplies the language and the `.hnl` resolver (ADR-0128 §0.8).
    pub fn with_lint_context(mut self, ctx: LintContext) -> Self {
        self.lint = Some(ctx);
        self
    }

    /// Registers a completion plugin.
    pub fn with_completion_plugin(mut self, plugin: Box<dyn CompletionPlugin>) -> Self {
        self.completion = Some(plugin);
        self
    }

    /// Registers a goto-definition plugin.
    pub fn with_goto_def_plugin(mut self, plugin: Box<dyn GotoDefPlugin>) -> Self {
        self.goto_def = Some(plugin);
        self
    }

    /// Registers a document-symbols plugin.
    pub fn with_document_symbols_plugin(mut self, plugin: Box<dyn DocumentSymbolsPlugin>) -> Self {
        self.document_symbols = Some(plugin);
        self
    }

    /// Registers an incremental-sync plugin.
    pub fn with_incremental_sync_plugin(mut self, plugin: Box<dyn IncrementalSyncPlugin>) -> Self {
        self.incremental_sync = Some(plugin);
        self
    }

    /// Registers a semantic-tokens plugin.
    pub fn with_semantic_tokens_plugin(mut self, plugin: Box<dyn SemanticTokensPlugin>) -> Self {
        self.semantic_tokens = Some(plugin);
        self
    }

    /// Registers a hover plugin.
    pub fn with_hover_plugin(mut self, plugin: Box<dyn HoverPlugin>) -> Self {
        self.hover = Some(plugin);
        self
    }

    /// Registers a signature-help plugin.
    pub fn with_signature_help_plugin(mut self, plugin: Box<dyn SignatureHelpPlugin>) -> Self {
        self.signature_help = Some(plugin);
        self
    }

    /// Registers a folding-range plugin.
    pub fn with_folding_range_plugin(mut self, plugin: Box<dyn FoldingRangePlugin>) -> Self {
        self.folding_range = Some(plugin);
        self
    }

    /// Returns the CLI commands registered by loaded plugins.
    pub fn cli_commands(&self) -> Vec<crate::command::CommandSpec> {
        Vec::new()
    }

    /// Runs a CLI command by name with the provided arguments.
    pub fn run_cli(&mut self, _name: &str, _args: &[String]) -> Result<String, PlatformError> {
        Err(PlatformError::NotImplemented)
    }

    /// Returns completion candidates at `pos` in `source` for the file at `path`.
    pub fn complete(
        &self,
        source: &str,
        pos: usize,
        path: &str,
    ) -> Result<Vec<CompletionItem>, PlatformError> {
        match &self.completion {
            Some(plugin) => {
                let resp = plugin.run(&CompletionRequest {
                    source: source.to_string(),
                    pos,
                    path: path.to_string(),
                })?;
                Ok(resp.0)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Returns the source span of the definition for the identifier at `pos`.
    pub fn goto_def(&self, request: &GotoDefRequest) -> Result<Option<Location>, PlatformError> {
        match &self.goto_def {
            Some(plugin) => {
                let resp = plugin.run(request)?;
                Ok(resp.0)
            }
            None => Ok(None),
        }
    }

    /// Returns the top-level document symbols for `source`.
    pub fn document_symbols(&self, source: &str) -> Result<Vec<DocumentSymbol>, PlatformError> {
        match &self.document_symbols {
            Some(plugin) => {
                let resp = plugin.run(&DocumentSymbolsRequest(source.to_string()))?;
                Ok(resp.0)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Applies an incremental `TextContentChange` to `source` and returns
    /// the resulting string.
    pub fn change(
        &mut self,
        source: &str,
        delta: TextContentChange,
    ) -> Result<String, PlatformError> {
        match &self.incremental_sync {
            Some(plugin) => {
                let resp = plugin.run(&IncrementalSyncRequest {
                    source: source.to_string(),
                    change: delta,
                })?;
                Ok(resp.0)
            }
            None => Ok(source.to_string()),
        }
    }

    /// Returns semantic tokens for `source`.
    pub fn semantic_tokens(&self, source: &str) -> Result<Vec<SemanticToken>, PlatformError> {
        match &self.semantic_tokens {
            Some(plugin) => {
                let resp = plugin.run(&SemanticTokensRequest {
                    source: source.to_string(),
                })?;
                Ok(resp.0)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Returns the semantic token legend from the configured plugin, or a
    /// default legend if no plugin is set.
    pub fn semantic_tokens_legend(&self) -> SemanticTokenLegend {
        match &self.semantic_tokens {
            Some(plugin) => plugin.legend(),
            None => SemanticTokenLegend::default(),
        }
    }

    /// Formats `source` with the shared data-driven formatter
    /// ([`handl_plugin_ext_fmt`], ADR-0044).
    ///
    /// The caller supplies the [`Language`] so the formatter and parser are
    /// fully data-driven — no hardcoded kind strings. This is the same
    /// implementation `hnl fmt` dispatches to; the host does not re-implement
    /// the lex → parse → render pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Plugin`] if lexing, parsing, or formatting
    /// fails.
    pub fn format(&self, lang: &Language, source: &str) -> Result<String, PlatformError> {
        handl_plugin_ext_fmt::format_source_with_language(lang, source)
            .map_err(|e| PlatformError::Plugin(e.to_string()))
    }

    /// Parses `source` and returns diagnostics for the resulting module.
    ///
    /// Delegates to the shared lint implementation
    /// ([`handl_plugin_ext_lint`]) over the injected [`LintContext`], which
    /// carries the language plus the handl-native resolver (ADR-0097) wired
    /// from the host kit's registered `.hnl` modules. Parse errors become
    /// error-level diagnostics; a host that never injected a context gets a
    /// single error diagnostic rather than silently clean output.
    ///
    /// The returned diagnostics are the shared `plugin-ext-diag` model, so
    /// they carry the rule name, line/column span and any machine-applicable
    /// fix.
    pub fn diagnose(&self, source: &str) -> Vec<Diagnostic> {
        match &self.lint {
            Some(ctx) => match handl_plugin_ext_lint::lint_source_with(ctx, source) {
                Ok(diags) => diags,
                Err(e) => vec![error_diagnostic(&e.to_string())],
            },
            None => vec![error_diagnostic(
                "no lint context configured (with_lint_context)",
            )],
        }
    }

    /// Returns a markdown hover string for the token at `request.pos` in
    /// `request.source`.
    pub fn hover(&self, request: &HoverRequest) -> Result<HoverResponse, PlatformError> {
        match &self.hover {
            Some(plugin) => plugin.run(request),
            None => Ok(HoverResponse(None)),
        }
    }

    /// Returns signature help for the call enclosing `request.pos` in
    /// `request.source`.
    pub fn signature_help(
        &self,
        request: &SignatureHelpRequest,
    ) -> Result<SignatureHelpResponse, PlatformError> {
        match &self.signature_help {
            Some(plugin) => plugin.run(request),
            None => Ok(SignatureHelpResponse::empty()),
        }
    }

    /// Returns folding ranges for `source`.
    pub fn folding_ranges(&self, source: &str) -> Result<Vec<FoldingRange>, PlatformError> {
        match &self.folding_range {
            Some(plugin) => {
                let resp = plugin.run(&FoldingRangeRequest {
                    source: source.to_string(),
                })?;
                Ok(resp.0)
            }
            None => Ok(Vec::new()),
        }
    }
}
