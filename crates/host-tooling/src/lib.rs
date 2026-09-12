//! Tooling / LSP host stubs; full implementation pending migration.

use handl_language::Language;
use handl_platform::{PlatformError, Plugin};

/// A diagnostic message with severity and optional source span.
///
/// This is a local replacement for the former `handl_plugin_diag_core::Diagnostic`
/// type, kept here so the `ToolingHost::diagnose` API remains stable while the
/// old plugin-diag-core crate is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Diagnostic message.
    pub message: String,
    /// Optional source span (byte offsets).
    pub span: Option<handl_platform::Span>,
}

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    /// Error-level diagnostic.
    Error,
    /// Warning-level diagnostic.
    Warning,
    /// Info-level diagnostic.
    Info,
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
pub struct ToolingHost<K: Plugin> {
    kit: K,
    completion: Option<Box<dyn CompletionPlugin>>,
    goto_def: Option<Box<dyn GotoDefPlugin>>,
    document_symbols: Option<Box<dyn DocumentSymbolsPlugin>>,
    incremental_sync: Option<Box<dyn IncrementalSyncPlugin>>,
    semantic_tokens: Option<Box<dyn SemanticTokensPlugin>>,
    hover: Option<Box<dyn HoverPlugin>>,
}

impl<K: Plugin> ToolingHost<K> {
    /// Creates a new tooling host with the given kit.
    pub fn new(kit: K) -> Self {
        Self {
            kit,
            completion: None,
            goto_def: None,
            document_symbols: None,
            incremental_sync: Some(Box::new(BasicIncrementalSync::new())),
            semantic_tokens: None,
            hover: None,
        }
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

    /// Returns a reference to the configured kit.
    pub fn kit(&self) -> &K {
        &self.kit
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

    /// Parses `source` using the data-driven lex + Pratt parse pipeline and
    /// returns a formatted canonical source string via [`FormatEngine`].
    ///
    /// The caller supplies the [`Language`] so the formatter and parser are
    /// fully data-driven — no hardcoded kind strings.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::Plugin`] if lexing, parsing, or formatting
    /// fails.
    pub fn format(&self, lang: &Language, source: &str) -> Result<String, PlatformError> {
        use handl_language::AstRef;
        use handl_plugin_engine_fmt::FormatEngine;
        use handl_plugin_engine_lex_pattern::lex;
        use handl_plugin_engine_parse::ParserEngine;
        use handl_plugin_engine_parse::ParserRequest;
        use handl_plugin_engine_parse_pratt::PrattEngine;

        let tokens = lex(lang, source).map_err(|e| PlatformError::Plugin(e.to_string()))?;
        if tokens.as_slice().is_empty() {
            return Ok(String::new());
        }
        let engine = PrattEngine::new();
        let req = ParserRequest::new(tokens);
        let ast = engine
            .parse(lang, &req)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;
        if ast.is_empty() {
            return Ok(String::new());
        }
        // Root = last node pushed (Pratt convention).
        let root = AstRef::new(ast.len() - 1);
        FormatEngine::new()
            .format(lang, &ast, root)
            .map_err(|e| PlatformError::Plugin(e.to_string()))
    }

    /// Parses `source` and returns diagnostics for the resulting module.
    ///
    /// Lexes and parses `source` with the canonical kit, then runs the
    /// data-driven [`handl_plugin_engine_lint::LintEngine`] over the
    /// resulting AST. Parse errors are converted to error-level diagnostics;
    /// lint diagnostics are mapped to the host-tooling [`Diagnostic`] type.
    pub fn diagnose(&self, source: &str) -> Vec<Diagnostic> {
        let kit = match handl_kit_canonical::Kit::new() {
            Ok(kit) => kit,
            Err(e) => {
                return vec![Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!("failed to assemble canonical kit: {e}"),
                    span: None,
                }];
            }
        };
        let ast = match kit.parse(source) {
            Ok(ast) => ast,
            Err(e) => {
                return vec![Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!("parse error: {e}"),
                    span: None,
                }];
            }
        };
        if ast.is_empty() {
            return Vec::new();
        }
        let root = handl_language::AstRef::new(ast.len() - 1);
        let lang = kit.runtime().lang();
        match handl_plugin_engine_lint::LintEngine::new()
            .with_canonical_lint_natives()
            .lint(lang, &ast, root)
        {
            Ok(lint_diags) => lint_diags
                .into_iter()
                .map(|d| Diagnostic {
                    level: match d.level {
                        handl_plugin_engine_lint::DiagnosticLevel::Error => DiagnosticLevel::Error,
                        handl_plugin_engine_lint::DiagnosticLevel::Warning => {
                            DiagnosticLevel::Warning
                        }
                        handl_plugin_engine_lint::DiagnosticLevel::Info => DiagnosticLevel::Info,
                    },
                    message: d.message,
                    span: d.span.map(|s| handl_platform::Span {
                        start: s.start,
                        end: s.end,
                    }),
                })
                .collect(),
            Err(e) => vec![Diagnostic {
                level: DiagnosticLevel::Error,
                message: format!("lint error: {e}"),
                span: None,
            }],
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
}
