//! Canonical expression kit: a thin composition of handl expression plugins.

use handl_ast_canonical::Expr;
use handl_expr::{ExpressionPipeline, ModuleOutput, ModulePipeline, ParseResult, Stmt};
use handl_lexer::Token;
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_runtime::{EvalOutput, RuntimePlugin, TypecheckedOutput, TypecheckPlugin};
use handl_vm_values_json::NonFunctionValue;

pub mod host;
pub use host::{CanonicalHost, PluginBundle};

pub mod kit_manifest;
pub use kit_manifest::{canonical_manifest, KitManifest};

/// A ready-to-use canonical handl expression kit.
pub struct CanonicalKit {
    module: ModulePipeline,
    typecheck: Option<Box<dyn TypecheckPlugin>>,
    runtime: Option<Box<dyn RuntimePlugin>>,
}

impl CanonicalKit {
    /// Builds a `CanonicalKit` from an already-configured `ExpressionPipeline`.
    pub fn from_pipeline(pipeline: ExpressionPipeline) -> Self {
        Self::from_module(ModulePipeline::new(pipeline))
    }

    /// Builds a `CanonicalKit` from an already-configured `ModulePipeline`.
    pub fn from_module(module: ModulePipeline) -> Self {
        Self::from_module_with(module, None, None)
    }

    /// Builds a `CanonicalKit` from a `ModulePipeline` and optional
    /// typecheck / runtime plugins.
    pub fn from_module_with(
        module: ModulePipeline,
        typecheck: Option<Box<dyn TypecheckPlugin>>,
        runtime: Option<Box<dyn RuntimePlugin>>,
    ) -> Self {
        Self {
            module,
            typecheck,
            runtime,
        }
    }

    /// Builds the canonical expression kit from the default manifest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lexes `input` into a token stream using the configured token rules.
    pub fn lex(&self, input: &str) -> Vec<Token<CanonicalKind>> {
        self.module.pipeline.lex(input).expect("canonical lex failed")
    }

    /// Parses a canonical expression from `input`.
    ///
    /// Comments are kept in the token stream as first-class trivia; the parser
    /// skips them during expression parsing.
    pub fn parse(&self, input: &str) -> Result<Expr<NonFunctionValue>, PlatformError> {
        let tokens = self.module.pipeline.lex(input)?;
        self.module.pipeline.parse_with_source(tokens, input)
    }

    /// Parses a canonical expression from `input`, recovering from errors.
    pub fn parse_recovered(&self, input: &str) -> ParseResult<Expr<NonFunctionValue>> {
        self.module.pipeline.parse_recovered(input)
    }

    /// Parses a canonical module from `input`.
    pub fn parse_module(&self, input: &str) -> Result<Vec<Stmt>, PlatformError> {
        self.module.parse_module(input)
    }

    /// Parses a canonical module from `input` with error recovery.
    pub fn parse_module_recovered(&self, input: &str) -> ModuleOutput {
        self.module.parse_module_with_recovery(input)
    }

    /// Typecheck a sequence of statements using the registered plugin.
    pub fn typecheck(&self, stmts: &[Stmt]) -> Result<TypecheckedOutput, PlatformError> {
        match self.typecheck.as_ref() {
            Some(plugin) => plugin.typecheck(stmts),
            None => Err(PlatformError::UnknownCapability("typecheck".into())),
        }
    }

    /// Evaluate a sequence of statements using the registered runtime plugin.
    pub fn eval(&self, stmts: &[Stmt]) -> Result<EvalOutput, PlatformError> {
        match self.runtime.as_ref() {
            Some(plugin) => plugin.eval(stmts),
            None => Err(PlatformError::UnknownCapability("runtime".into())),
        }
    }
}

impl Default for CanonicalKit {
    fn default() -> Self {
        CanonicalHost::default()
            .load_kit(&canonical_manifest())
            .expect("canonical manifest should always resolve")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_ast_canonical::{Expr, Pattern};
    use handl_expr::{DiagnosticLevel, Stmt, StmtKind};

    fn parse(input: &str) -> Expr<NonFunctionValue> {
        CanonicalKit::new().parse(input).unwrap()
    }

    #[test]
    fn number_literal() {
        assert_eq!(parse("42"), Expr::Value(NonFunctionValue::new(42.0)));
    }

    #[test]
    fn string_literal() {
        assert_eq!(
            parse("\"hi\""),
            Expr::Value(NonFunctionValue::new("hi".to_string()))
        );
    }

    #[test]
    fn identifier() {
        assert_eq!(parse("add"), Expr::var("add"));
    }

    #[test]
    fn call() {
        assert_eq!(
            parse("add(1, 2)"),
            Expr::call(
                "add",
                vec![
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::Value(NonFunctionValue::new(2.0)),
                ],
            )
        );
    }

    #[test]
    fn index() {
        assert_eq!(
            parse("arr[0]"),
            Expr::index(
                Expr::var("arr"),
                Expr::Value(NonFunctionValue::new(0.0)),
            )
        );
    }

    #[test]
    fn dotted_identifier() {
        assert_eq!(parse("obj.key"), Expr::var("obj.key"));
    }

    #[test]
    fn field_access_after_index() {
        assert_eq!(
            parse("arr[0].name"),
            Expr::field_access(
                Expr::index(Expr::var("arr"), Expr::Value(NonFunctionValue::new(0.0))),
                "name",
            )
        );
    }

    #[test]
    fn mood_is() {
        assert_eq!(
            parse("x is int"),
            Expr::call(
                "mood.int",
                vec![
                    Expr::var("x"),
                    Expr::Value(NonFunctionValue::new("int".to_string())),
                ],
            )
        );
    }

    #[test]
    fn assignment() {
        assert_eq!(
            parse("a = 1"),
            Expr::assign(
                Pattern::Identifier("a".to_string()),
                Expr::Value(NonFunctionValue::new(1.0)),
            )
        );
    }

    #[test]
    fn pipe() {
        assert_eq!(
            parse("1 |> add"),
            Expr::call(
                "fn.pipe",
                vec![
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::var("add"),
                ],
            )
        );
    }

    #[test]
    fn array_literal() {
        assert_eq!(
            parse("[1, 2]"),
            Expr::array(vec![
                Expr::Value(NonFunctionValue::new(1.0)),
                Expr::Value(NonFunctionValue::new(2.0)),
            ])
        );
    }

    #[test]
    fn not_expression() {
        assert_eq!(
            parse("not true"),
            Expr::call(
                "bool.not",
                vec![Expr::Value(NonFunctionValue::new(true))],
            )
        );
    }

    #[test]
    fn function_literal() {
        assert_eq!(
            parse("add(a) -> a"),
            Expr::assign(
                Pattern::Identifier("add".to_string()),
                Expr::function(
                    Vec::new(),
                    vec![(Pattern::Identifier("a".to_string()), None)],
                    None,
                    "a",
                ),
            )
        );
    }

    #[test]
    fn comment_skipped_before_value() {
        assert_eq!(
            parse("// hello\n42"),
            Expr::Value(NonFunctionValue::new(42.0))
        );
    }

    #[test]
    fn comment_skipped_in_pipe() {
        assert_eq!(
            parse("1 /* comment */ |> add"),
            Expr::call(
                "fn.pipe",
                vec![
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::var("add"),
                ],
            )
        );
    }

    #[test]
    fn comment_skipped_in_array() {
        assert_eq!(
            parse("[1, /* inline */ 2]"),
            Expr::array(vec![
                Expr::Value(NonFunctionValue::new(1.0)),
                Expr::Value(NonFunctionValue::new(2.0)),
            ])
        );
    }

    #[test]
    fn match_expression() {
        assert_eq!(
            parse("match x { 1 -> 2 }"),
            Expr::call(
                "fn.match",
                vec![
                    Expr::var("x"),
                    Expr::array(vec![
                        Expr::Value(NonFunctionValue::new(1.0)),
                        Expr::function(Vec::new(), Vec::new(), None, "2"),
                    ]),
                ],
            )
        );
    }

    #[test]
    fn performance_long_pipe() {
        use std::time::Instant;

        let mut input = "1".to_string();
        for _ in 0..333 {
            input.push_str(" |> a");
        }

        let kit = CanonicalKit::new();
        let start = Instant::now();
        let expr = kit.parse(&input).unwrap();
        let elapsed = start.elapsed();

        eprintln!("parsed {} tokens in {:?}", 1 + 333 * 3, elapsed);

        // The exact shape is a deeply nested `fn.pipe` call; we just
        // confirm it returned an expression and completed.
        assert!(matches!(expr, Expr::Call { .. }));
    }

    #[test]
    fn handle_expression() {
        use handl_ast_canonical::HandlerClause;
        assert_eq!(
            parse("handle add as add"),
            Expr::handle(
                Expr::var("add"),
                vec![HandlerClause {
                    op_name: "add".to_string(),
                    params: Vec::new(),
                    continuation_name: String::new(),
                    body: Expr::var("add"),
                }],
            )
        );
    }

    #[test]
    fn kit_from_minimal_manifest() {
        let manifest = KitManifest {
            id: "minimal".to_string(),
            version: "0.1.0".to_string(),
            plugins: vec![
                "numbers".to_string(),
                "strings".to_string(),
                "identifiers".to_string(),
                "delimiters".to_string(),
                "operators".to_string(),
                "primary-values".to_string(),
                "primary-identifiers".to_string(),
                "postfix-calls".to_string(),
                "infix-assign".to_string(),
            ],
        };

        let kit = CanonicalHost::default().load_kit(&manifest).unwrap();
        assert_eq!(
            kit.parse("a = 1").unwrap(),
            Expr::assign(
                Pattern::Identifier("a".to_string()),
                Expr::Value(NonFunctionValue::new(1.0)),
            )
        );
    }

    #[test]
    fn kit_from_default_manifest_parses_pipe_call() {
        let kit = CanonicalKit::new();
        let expr = kit.parse("1 |> add(2, 3)").unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "fn.pipe",
                vec![
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::call(
                        "add",
                        vec![
                            Expr::Value(NonFunctionValue::new(2.0)),
                            Expr::Value(NonFunctionValue::new(3.0)),
                        ],
                    ),
                ],
            )
        );
    }

    fn parse_module(input: &str) -> Vec<Stmt> {
        CanonicalKit::new().parse_module_recovered(input).stmts
    }

    fn parse_module_output(input: &str) -> ModuleOutput {
        CanonicalKit::new().parse_module_recovered(input)
    }

    #[test]
    fn module_let() {
        assert_eq!(
            parse_module("let a = 1;"),
            vec![Stmt::new(StmtKind::Let {
                pattern: Pattern::Identifier("a".to_string()),
                value: Expr::Value(NonFunctionValue::new(1.0)),
            })]
        );
    }

    #[test]
    fn module_let_and_const() {
        assert_eq!(
            parse_module("let a = 1; const b = 2;"),
            vec![
                Stmt::new(StmtKind::Let {
                    pattern: Pattern::Identifier("a".to_string()),
                    value: Expr::Value(NonFunctionValue::new(1.0)),
                }),
                Stmt::new(StmtKind::Const {
                    pattern: Pattern::Identifier("b".to_string()),
                    value: Expr::Value(NonFunctionValue::new(2.0)),
                }),
            ]
        );
    }

    #[test]
    fn module_import_export() {
        assert_eq!(
            parse_module("import \"path\"; export a;"),
            vec![
                Stmt::new(StmtKind::Import {
                    source: Expr::Value(NonFunctionValue::new("path".to_string())),
                }),
                Stmt::new(StmtKind::Export {
                    value: Expr::var("a"),
                }),
            ]
        );
    }

    #[test]
    fn module_let_then_expr() {
        assert_eq!(
            parse_module("let a = 1; a"),
            vec![
                Stmt::new(StmtKind::Let {
                    pattern: Pattern::Identifier("a".to_string()),
                    value: Expr::Value(NonFunctionValue::new(1.0)),
                }),
                Stmt::new(StmtKind::Expr(Expr::var("a"))),
            ]
        );
    }

    #[test]
    fn module_declaration() {
        assert_eq!(
            parse_module("module m;"),
            vec![Stmt::new(StmtKind::Module {
                name: "m".to_string(),
                body: None,
            })]
        );
    }

    #[test]
    fn module_recovers_after_bad_let() {
        let output = parse_module_output("let a = ; const b = 2;");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].level, handl_expr::DiagnosticLevel::Error);
        assert_eq!(
            output.stmts,
            vec![Stmt::new(StmtKind::Const {
                pattern: Pattern::Identifier("b".to_string()),
                value: Expr::Value(NonFunctionValue::new(2.0)),
            })]
        );
    }

    #[test]
    fn module_recovers_after_unclosed_call() {
        let output = parse_module_output("let a = 1; f(");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].level, handl_expr::DiagnosticLevel::Error);
        assert_eq!(
            output.stmts,
            vec![Stmt::new(StmtKind::Let {
                pattern: Pattern::Identifier("a".to_string()),
                value: Expr::Value(NonFunctionValue::new(1.0)),
            })]
        );
    }

    #[test]
    fn module_recovers_after_bad_import() {
        let output = parse_module_output("import ; export a;");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].level, handl_expr::DiagnosticLevel::Error);
        assert_eq!(
            output.stmts,
            vec![Stmt::new(StmtKind::Export {
                value: Expr::var("a"),
            })]
        );
    }

    #[test]
    fn kit_typecheck_simple_value() {
        let kit = CanonicalKit::new();
        let stmts = vec![Stmt::new(StmtKind::Expr(Expr::Value(NonFunctionValue::new(42.0))))];
        let output = kit.typecheck(&stmts).unwrap();
        assert!(output.success);
    }

    #[test]
    fn kit_eval_simple_value() {
        let kit = CanonicalKit::new();
        let stmts = vec![Stmt::new(StmtKind::Expr(Expr::Value(NonFunctionValue::new(42.0))))];
        let output = kit.eval(&stmts).unwrap();
        assert_eq!(output.values.len(), 1);
        assert_eq!(output.values[0].as_number(), Some(42.0));
    }

    #[test]
    fn module_preserves_leading_comment() {
        let output = parse_module_output("// hello\nlet a = 1;");
        assert_eq!(output.stmts.len(), 1);
        assert_eq!(output.stmts[0].leading.len(), 1);
        assert!(matches!(
            output.stmts[0].leading[0].kind,
            handl_lexer_canonical::CanonicalKind::Comment(_)
        ));
        assert_eq!(
            output.stmts[0].kind,
            StmtKind::Let {
                pattern: Pattern::Identifier("a".to_string()),
                value: Expr::Value(NonFunctionValue::new(1.0)),
            }
        );
    }

    #[test]
    fn module_preserves_trailing_comment() {
        let output = parse_module_output("let a = 1 /* x */;");
        assert_eq!(output.stmts.len(), 1);
        assert_eq!(output.stmts[0].trailing.len(), 1);
        assert!(matches!(
            output.stmts[0].trailing[0].kind,
            handl_lexer_canonical::CanonicalKind::Comment(_)
        ));
        assert_eq!(
            output.stmts[0].kind,
            StmtKind::Let {
                pattern: Pattern::Identifier("a".to_string()),
                value: Expr::Value(NonFunctionValue::new(1.0)),
            }
        );
    }

    fn parse_expr_recovered(input: &str) -> handl_expr::ParseResult<Expr<NonFunctionValue>> {
        CanonicalKit::new().parse_recovered(input)
    }

    #[test]
    fn recovers_from_missing_rhs() {
        let r = parse_expr_recovered("1 |>");
        assert_eq!(r.value, Expr::Value(NonFunctionValue::new(1.0)));
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(r.diagnostics[0].level, DiagnosticLevel::Error);
    }

    #[test]
    fn recovers_from_trailing_comma_in_call() {
        let r = parse_expr_recovered("add(1,)");
        assert_eq!(
            r.value,
            Expr::call(
                "add",
                vec![Expr::Value(NonFunctionValue::new(1.0))],
            )
        );
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(r.diagnostics[0].level, DiagnosticLevel::Error);
    }

    #[test]
    fn recovers_from_unclosed_array() {
        let r = parse_expr_recovered("[1, 2,");
        assert_eq!(
            r.value,
            Expr::array(vec![
                Expr::Value(NonFunctionValue::new(1.0)),
                Expr::Value(NonFunctionValue::new(2.0)),
            ])
        );
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(r.diagnostics[0].level, DiagnosticLevel::Error);
    }

    #[test]
    fn recovers_from_broken_let_statement() {
        let output = CanonicalKit::new().parse_module_recovered("let a = ;");
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].level, DiagnosticLevel::Error);
        assert!(output.stmts.is_empty());
    }

    #[test]
    fn recovers_and_parses_next_statement() {
        let output = CanonicalKit::new().parse_module_recovered("let a = ;\n1");
        assert_eq!(
            output.stmts,
            vec![Stmt::new(StmtKind::Expr(Expr::Value(NonFunctionValue::new(1.0))))]
        );
        assert_eq!(output.diagnostics.len(), 1);
    }
}
