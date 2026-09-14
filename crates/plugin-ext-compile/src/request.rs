//! Request builder for the `hnl compile` CLI path (ADR-0114 Phase 14).

use std::path::PathBuf;

use handl_language::{Ast, AstNode, AstRef, Language, Span};
use handl_plugin_compile::{CompileError, CompilerInput};
use plugin_engine::Term;
use plugin_engine_typecheck::TypecheckReport;
use serde_json::Value;

/// A parsed `hnl compile <target> <file>` invocation.
#[derive(Debug, Clone)]
pub struct CompileFileRequest {
    /// The compilation target (e.g. `js`, `llvm`).
    pub target: String,
    /// The input source file.
    pub path: PathBuf,
    /// Optional output file path (otherwise stdout).
    pub output: Option<PathBuf>,
    /// Whether to emit a source map.
    pub source_map: bool,
    /// Whether to emit debug information.
    pub debug: bool,
    /// Target-specific output format (e.g. `esm`, `commonjs`, `iife`, `inline`, `o`, `ir`).
    pub format: Option<String>,
}

impl CompileFileRequest {
    /// Parse `args` as `compile <target> <file> [options]`.
    pub fn from_args(args: &[String]) -> Result<Self, CompileError> {
        let mut positional = Vec::new();
        let mut output = None;
        let mut source_map = false;
        let mut debug = false;
        let mut format = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--output" => {
                    i += 1;
                    let v = args.get(i).ok_or_else(|| {
                        CompileError::InvalidInput("--output requires a value".to_string())
                    })?;
                    output = Some(PathBuf::from(v));
                }
                "--source-map" => source_map = true,
                "--debug" => debug = true,
                "--format" => {
                    i += 1;
                    let v = args.get(i).ok_or_else(|| {
                        CompileError::InvalidInput("--format requires a value".to_string())
                    })?;
                    format = Some(v.clone());
                }
                "--" => {}
                s if s.starts_with("--") => {
                    return Err(CompileError::InvalidInput(format!("unknown flag `{s}`")));
                }
                s => positional.push(s.to_string()),
            }
            i += 1;
        }

        if positional.len() < 2 {
            return Err(CompileError::InvalidInput(
                "usage: compile <target> <file> [options]".to_string(),
            ));
        }
        if positional.len() > 2 {
            return Err(CompileError::InvalidInput(
                "too many positional arguments".to_string(),
            ));
        }

        Ok(Self {
            target: positional[0].clone(),
            path: PathBuf::from(&positional[1]),
            output,
            source_map,
            debug,
            format,
        })
    }

    /// Build a [`CompilerInput`] from the parsed request and caller-supplied data.
    pub fn build_input(
        &self,
        src: &str,
        ast: &Ast,
        lang: &Language,
        term: &Term,
        report: &TypecheckReport,
    ) -> CompilerInput {
        CompilerInput::new(
            src.to_string(),
            ast.clone(),
            AstRef::new(0),
            lang.clone(),
            term.clone(),
            report.clone(),
        )
    }

    /// Return the root evaluation [`Term`] for `lang`, falling back to a null constant.
    pub fn root_term_for(lang: &Language) -> Term {
        lang.eval
            .iter()
            .next()
            .map(|def| def.scheme.clone())
            .unwrap_or_else(|| Term::const_value(Value::Null))
    }

    /// Build a minimal `Ast` containing a single `module` root node spanning `src`.
    pub fn build_ast(src: &str) -> Ast {
        let mut ast = Ast::new();
        let node = AstNode::new("module").with_span(Span {
            start: 0,
            end: src.len(),
        });
        ast.push(node);
        ast
    }
}
