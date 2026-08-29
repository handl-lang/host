//! Tooling / LSP host stubs; full implementation pending migration.

use handl_expr::{Diagnostic, Formatter};
use handl_loader::LoaderRegistry;
use handl_platform::{PlatformError, Plugin, PluginManifest};
use handl_plugin_fmt_stmt::StatementFormatter;
use handl_plugin_fmt_trivia::TriviaFormatter;


pub mod command;

pub mod config;
pub use config::*;

pub mod lsp;
pub use lsp::*;

/// Tooling / LSP host that decouples formatter and diagnostics from parser internals.
#[allow(dead_code)]
pub struct ToolingHost<K: Plugin> {
    kit: K,
    formatter: Box<dyn Formatter>,
    trivia_formatter: TriviaFormatter,
    statement_formatter: StatementFormatter,
    completion: Option<Box<dyn CompletionPlugin>>,
    goto_def: Option<Box<dyn GotoDefPlugin>>,
    document_symbols: Option<Box<dyn DocumentSymbolsPlugin>>,
    incremental_sync: Option<Box<dyn IncrementalSyncPlugin>>,
    loaders: LoaderRegistry,
}

impl<K: Plugin> ToolingHost<K> {
    /// Creates a new tooling host with the given kit and formatter.
    pub fn new(kit: K, formatter: Box<dyn Formatter>) -> Self {
        Self {
            kit,
            formatter,
            trivia_formatter: TriviaFormatter::new(),
            statement_formatter: StatementFormatter::new(),
            completion: None,
            goto_def: None,
            document_symbols: None,
            incremental_sync: Some(Box::new(BasicIncrementalSync::new())),
            loaders: LoaderRegistry::new(),
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

    /// Sets the loader registry used to load external plugins.
    pub fn with_loader_registry(mut self, registry: LoaderRegistry) -> Self {
        self.loaders = registry;
        self
    }

    /// Returns a reference to the configured kit.
    pub fn kit(&self) -> &K {
        &self.kit
    }

    /// Loads an external plugin using the loader registry.
    pub fn load_plugin(&mut self, manifest: &PluginManifest) -> Result<(), PlatformError> {
        self.loaders.load(manifest)?;
        Ok(())
    }

    /// Returns the CLI commands registered by loaded plugins.
    pub fn cli_commands(&self) -> Vec<crate::command::CommandSpec> {
        Vec::new()
    }

    /// Runs a CLI command by name with the provided arguments.
    pub fn run_cli(
        &mut self,
        _name: &str,
        _args: &[String],
    ) -> Result<String, PlatformError> {
        Err(PlatformError::NotImplemented)
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
    pub fn format(&self, _source: &str) -> Result<String, PlatformError> {
        Err(PlatformError::NotImplemented)
    }

    /// Parses `source` and returns diagnostics for the resulting module.
    pub fn diagnose(&self, _source: &str) -> Vec<Diagnostic> {
        Vec::new()
    }

    /// Hover placeholder: returns the formatted expression for the whole source.
    pub fn hover(&self, _source: &str, _pos: usize) -> Option<String> {
        None
    }
}


