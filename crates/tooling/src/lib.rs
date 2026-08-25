use handl_ast_canonical::Expr;
use handl_expr::{Diagnostic, Diagnostics, Formatter, Stmt};
use handl_kit_canonical::CanonicalKit;
use handl_platform::PlatformError;
use handl_plugin_fmt_expression::ExpressionFormatter;
use handl_plugin_fmt_statements::StatementFormatter;
use handl_plugin_fmt_trivia::TriviaFormatter;
use handl_vm_values_json::NonFunctionValue;

pub mod command;
pub use command::*;

pub mod lsp;
pub use lsp::*;

/// Tooling / LSP host that decouples formatter and diagnostics from parser internals.
///
/// It owns a `CanonicalKit` for parsing and uses the `Formatter` and `Diagnostics`
/// extension points for formatting and linting. LSP features are supplied by
/// optional decoupled plugins.
#[allow(dead_code)]
pub struct ToolingHost {
    kit: CanonicalKit,
    formatter: Box<dyn Formatter>,
    trivia_formatter: TriviaFormatter,
    statement_formatter: StatementFormatter,
    diagnostics: Box<dyn Diagnostics>,
    completion: Option<Box<dyn CompletionPlugin>>,
    goto_def: Option<Box<dyn GotoDefPlugin>>,
    document_symbols: Option<Box<dyn DocumentSymbolsPlugin>>,
    incremental_sync: Option<Box<dyn IncrementalSyncPlugin>>,
    command_providers: Vec<Box<dyn CommandProvider>>,
}

impl ToolingHost {
    /// Creates a new tooling host with the given parser, formatter and diagnostics.
    pub fn new(kit: CanonicalKit, formatter: Box<dyn Formatter>, diagnostics: Box<dyn Diagnostics>) -> Self {
        Self {
            kit,
            formatter,
            trivia_formatter: TriviaFormatter::new(),
            statement_formatter: StatementFormatter::new(
                Box::new(ExpressionFormatter::new()),
                TriviaFormatter::new(),
            ),
            diagnostics,
            completion: None,
            goto_def: None,
            document_symbols: None,
            incremental_sync: Some(Box::new(BasicIncrementalSync::new())),
            command_providers: Vec::new(),
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

    /// Registers a command provider.
    pub fn with_command_provider(mut self, provider: Box<dyn CommandProvider>) -> Self {
        self.command_providers.push(provider);
        self
    }

    /// Returns the merged command specs from all registered command providers.
    pub fn cli_commands(&self) -> Vec<CommandSpec> {
        self.command_providers.iter().flat_map(|p| p.commands()).collect()
    }

    /// Dispatches a CLI command to the appropriate provider.
    pub fn run_cli(&self, command: &str, args: &[String]) -> Result<serde_json::Value, PlatformError> {
        for provider in &self.command_providers {
            for spec in provider.commands() {
                if spec.name == command || spec.aliases.iter().any(|a| a == command) {
                    return provider.run(command, args, self);
                }
            }
        }
        Err(PlatformError::UnknownCapability(command.into()))
    }

    /// Returns completion candidates at `pos` in `source`.
    pub fn complete(&self, source: &str, pos: usize) -> Result<Vec<CompletionItem>, PlatformError> {
        match &self.completion {
            Some(plugin) => {
                let resp = plugin.run(&CompletionRequest {
                    source: source.to_string(),
                    pos,
                })?;
                Ok(resp.0)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Returns the source span of the definition for the identifier at `pos`.
    pub fn goto_def(&self, source: &str, pos: usize) -> Result<Option<Location>, PlatformError> {
        match &self.goto_def {
            Some(plugin) => {
                let resp = plugin.run(&GotoDefRequest {
                    source: source.to_string(),
                    pos,
                })?;
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
    pub fn change(&mut self, source: &str, delta: TextContentChange) -> Result<String, PlatformError> {
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

    /// Parses `source` and returns a formatted canonical source string.
    pub fn format(&self, source: &str) -> Result<String, PlatformError> {
        let output = self.kit.parse_module_recovered(source);
        self.statement_formatter.format(&output.stmts)
    }

    /// Parses `source` and returns diagnostics for the resulting module.
    pub fn diagnose(&self, source: &str) -> Vec<Diagnostic> {
        let output = self.kit.parse_module_recovered(source);
        let mut diags = output.diagnostics;

        for stmt in &output.stmts {
            if let Some(expr) = stmt_expr(stmt) {
                diags.extend(self.diagnostics.diagnose(expr));
            }
        }

        diags
    }

    /// Hover placeholder: returns the formatted expression for the whole source.
    pub fn hover(&self, _source: &str, _pos: usize) -> Option<String> {
        // TODO: real position resolution
        None
    }
}

fn stmt_expr(stmt: &Stmt) -> Option<&Expr<NonFunctionValue>> {
    match &stmt.kind {
        handl_expr::StmtKind::Expr(expr)
        | handl_expr::StmtKind::Let { value: expr, .. }
        | handl_expr::StmtKind::Const { value: expr, .. }
        | handl_expr::StmtKind::Import { source: expr }
        | handl_expr::StmtKind::Export { value: expr }
        | handl_expr::StmtKind::Module {
            body: Some(expr), ..
        } => Some(expr),
        handl_expr::StmtKind::Module { body: None, .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_diag_expression::ExpressionDiagnostics;
    use handl_plugin_fmt_expression::ExpressionFormatter;

    fn host() -> ToolingHost {
        ToolingHost::new(
            CanonicalKit::new(),
            Box::new(ExpressionFormatter::new()),
            Box::new(ExpressionDiagnostics::new()),
        )
    }

    #[test]
    fn formats_pipe() {
        let h = host();
        let out = h.format("1 |> add").unwrap();
        assert_eq!(out, "1 |> add");
    }

    #[test]
    fn formats_let() {
        let h = host();
        let out = h.format("let a = 1").unwrap();
        assert_eq!(out, "let a = 1;");
    }

    #[test]
    fn formats_with_leading_comment() {
        let h = host();
        let out = h.format("// hello\nlet a = 1;").unwrap();
        assert_eq!(out, "// hello\nlet a = 1;");
    }

    #[test]
    fn diagnoses_double_negation() {
        let h = host();
        let diags = h.diagnose("not not true");
        assert!(!diags.is_empty());
    }

    #[test]
    fn hover_returns_none() {
        let h = host();
        assert!(h.hover("1", 0).is_none());
    }
}
