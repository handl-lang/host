//! Statement parsing support for handl modules.
//!
//! This module provides a `Stmt` value, a `StatementFragment` trait for
//! plugins, and a `ModulePipeline` that combines an `ExpressionPipeline`
//! with registered statement fragments.

use crate::context::ParseContext;
use crate::diagnostics::{Diagnostic, DiagnosticLevel};
use crate::pipeline::ExpressionPipeline;
use crate::token::{Span, Token};
use handl_ast_canonical::{Expr, Pattern};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// The payload of a top-level statement in a handl module.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `let <pattern> = <expr>`
    Let {
        pattern: Pattern,
        value: Expr<NonFunctionValue>,
    },
    /// `const <pattern> = <expr>`
    Const {
        pattern: Pattern,
        value: Expr<NonFunctionValue>,
    },
    /// `import <source>`
    Import {
        source: Expr<NonFunctionValue>,
    },
    /// `export <value>`
    Export {
        value: Expr<NonFunctionValue>,
    },
    /// `module <name>` or `module <name> = <expr>`
    Module {
        name: String,
        body: Option<Expr<NonFunctionValue>>,
    },
    /// A bare expression used as a statement.
    Expr(Expr<NonFunctionValue>),
}

/// A top-level statement in a handl module, carrying leading and trailing
/// comment trivia.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    /// The statement payload.
    pub kind: StmtKind,
    /// Comment tokens appearing before the statement.
    pub leading: Vec<Token>,
    /// Comment tokens appearing immediately after the statement.
    pub trailing: Vec<Token>,
}

impl Stmt {
    /// Creates a `Stmt` with empty trivia.
    pub fn new(kind: StmtKind) -> Self {
        Stmt {
            kind,
            leading: Vec::new(),
            trailing: Vec::new(),
        }
    }
}

/// The result of parsing a module with error recovery.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleOutput {
    /// The statements that were successfully parsed.
    pub stmts: Vec<Stmt>,
    /// Parse errors recovered from during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// A plugin-provided parser for a single kind of statement.
pub trait StatementFragment: Send + Sync {
    /// Attempts to parse a statement starting at the current token.
    ///
    /// Returns `None` if this fragment does not apply. `Some(Ok(_))` on
    /// success and `Some(Err(_))` on a recoverable parse error.
    fn try_statement(&self, ctx: &mut ParseContext) -> Option<Result<Stmt, PlatformError>>;
}

/// Converts a parsed primary expression into a binding pattern.
pub fn expr_to_pattern<V>(expr: &Expr<V>) -> Result<Pattern, PlatformError> {
    match expr {
        Expr::Var(name) => Ok(Pattern::Identifier(name.clone())),
        Expr::Array(elements) => Ok(Pattern::Array(
            elements
                .iter()
                .map(expr_to_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Expr::Object(entries) => Ok(Pattern::Object(
            entries
                .iter()
                .map(|(k, v)| expr_to_pattern(v).map(|p| (k.clone(), p)))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        _ => Err(PlatformError::Plugin("invalid pattern".to_string())),
    }
}

fn collect_comments(ctx: &mut ParseContext) -> Vec<Token> {
    let mut comments = Vec::new();
    while ctx.pos < ctx.tokens.len() {
        if matches!(ctx.tokens[ctx.pos].kind, CanonicalKind::Comment(_)) {
            comments.push(ctx.tokens[ctx.pos].clone());
            ctx.pos += 1;
        } else {
            break;
        }
    }
    comments
}

fn collect_trailing(ctx: &ParseContext) -> Vec<Token> {
    let mut count = 0;
    while count < ctx.pos
        && matches!(
            ctx.tokens[ctx.pos - 1 - count].kind,
            CanonicalKind::Comment(_)
        )
    {
        count += 1;
    }
    ctx.tokens[ctx.pos - count..ctx.pos].to_vec()
}

/// A pluggable pipeline for parsing whole modules.
///
/// Wraps an [`ExpressionPipeline`] and a set of registered statement
/// fragments. Statements are separated by `;` or end of input.
pub struct ModulePipeline {
    /// The underlying expression pipeline.
    pub pipeline: ExpressionPipeline,
    /// Registered statement fragments, tried in registration order.
    pub statements: Vec<Box<dyn StatementFragment>>,
}

impl ModulePipeline {
    /// Creates a `ModulePipeline` wrapping the given expression pipeline.
    pub fn new(pipeline: ExpressionPipeline) -> Self {
        Self {
            pipeline,
            statements: Vec::new(),
        }
    }

    /// Registers a statement fragment.
    pub fn register_statement(&mut self, fragment: Box<dyn StatementFragment>) {
        self.statements.push(fragment);
    }

    /// Lexes `input` into a token stream.
    pub fn lex(&self, input: &str) -> Result<Vec<Token>, PlatformError> {
        self.pipeline.lex(input)
    }

    /// Parses `input` as a single expression using the underlying pipeline.
    pub fn parse_expr(&self, input: &str) -> Result<Expr<NonFunctionValue>, PlatformError> {
        let tokens = self.pipeline.lex(input)?;
        self.pipeline.parse_with_source(tokens, input)
    }

    /// Parses `input` as a sequence of statements.
    ///
    /// Leading and trailing `Comment` tokens are captured on each statement.
    /// Consumes optional `;` terminators after each statement.
    pub fn parse_module(&self, input: &str) -> Result<Vec<Stmt>, PlatformError> {
        let tokens = match self.pipeline.lex(input) {
            Ok(tokens) => tokens,
            Err(e) => return Err(e),
        };

        let mut ctx = ParseContext::new(&tokens, input, &self.pipeline);
        let mut stmts = Vec::new();

        loop {
            let leading = collect_comments(&mut ctx);
            if ctx.at_end() {
                break;
            }

            let mut stmt: Option<Stmt> = None;
            for fragment in &self.statements {
                if let Some(result) = fragment.try_statement(&mut ctx) {
                    stmt = Some(result?);
                    break;
                }
            }

            let mut stmt = match stmt {
                Some(s) => s,
                None => {
                    let expr = ctx.parse_expr()?;
                    Stmt::new(StmtKind::Expr(expr))
                }
            };

            stmt.leading = leading;
            stmt.trailing = collect_trailing(&ctx);
            stmts.push(stmt);

            ctx.consume_if(CanonicalKind::Semicolon);
        }

        Ok(stmts)
    }

    /// Parses `input` as a sequence of statements with error recovery.
    ///
    /// Statements that fail to parse are reported as `Diagnostic` records and
    /// skipped up to the next `;` (or end of input). Parsing continues for any
    /// remaining statements. Leading and trailing `Comment` tokens are attached
    /// to successfully parsed statements.
    pub fn parse_module_with_recovery(&self, input: &str) -> ModuleOutput {
        let tokens = match self.pipeline.lex(input) {
            Ok(tokens) => tokens,
            Err(e) => {
                return ModuleOutput {
                    stmts: Vec::new(),
                    diagnostics: vec![Diagnostic {
                        level: DiagnosticLevel::Error,
                        message: e.to_string(),
                        span: Some(Span::at(input.len())),
                    }],
                }
            }
        };

        let mut ctx = ParseContext::new(&tokens, input, &self.pipeline);
        let mut stmts = Vec::new();
        let mut diagnostics = Vec::new();

        loop {
            let mut leading = collect_comments(&mut ctx);
            if ctx.at_end() {
                break;
            }

            let mut matched = false;
            let mut recovered = false;

            for fragment in &self.statements {
                if let Some(result) = fragment.try_statement(&mut ctx) {
                    matched = true;
                    match result {
                        Ok(mut stmt) => {
                            stmt.leading = std::mem::take(&mut leading);
                            stmt.trailing = collect_trailing(&ctx);
                            stmts.push(stmt);
                        }
                        Err(e) => {
                            diagnostics.push(Diagnostic {
                                level: DiagnosticLevel::Error,
                                message: e.to_string(),
                                span: Some(ctx.current_span()),
                            });
                            ctx.seek_to(CanonicalKind::Semicolon);
                            ctx.consume_if(CanonicalKind::Semicolon);
                            recovered = true;
                        }
                    }
                    break;
                }
            }

            if !matched {
                match ctx.parse_expr() {
                    Ok(expr) => {
                        let mut stmt = Stmt::new(StmtKind::Expr(expr));
                        stmt.leading = std::mem::take(&mut leading);
                        stmt.trailing = collect_trailing(&ctx);
                        stmts.push(stmt);
                    }
                    Err(e) => {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            message: e.to_string(),
                            span: Some(ctx.current_span()),
                        });
                        ctx.seek_to(CanonicalKind::Semicolon);
                        ctx.consume_if(CanonicalKind::Semicolon);
                        recovered = true;
                    }
                }
            }

            if !recovered {
                ctx.consume_if(CanonicalKind::Semicolon);
            }
        }

        ModuleOutput { stmts, diagnostics }
    }
}
