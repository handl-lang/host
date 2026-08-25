use handl_expr::{ParseContext, StatementFragment, Stmt, StmtKind};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Statement fragment for `module <name>` or `module <name> = <expr>`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Module;

impl Module {
    /// Creates a new `module` statement fragment.
    pub fn new() -> Self {
        Self
    }
}

impl StatementFragment for Module {
    fn try_statement(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Stmt, PlatformError>> {
        let token = ctx.peek().cloned()?;
        match &token.kind {
            CanonicalKind::Identifier(name) if name == "module" => {}
            _ => return None,
        }
        ctx.consume();

        let name = match ctx.consume() {
            Some(t) => match &t.kind {
                CanonicalKind::Identifier(n) => n.clone(),
                _ => return Some(Err(PlatformError::Plugin(
                    "expected module name".to_string(),
                ))),
            },
            None => return Some(Err(PlatformError::Plugin(
                "expected module name".to_string(),
            ))),
        };

        let token = ctx.peek().cloned();
        match token.map(|t| t.kind) {
            Some(CanonicalKind::Semicolon) => {
                ctx.consume();
                Some(Ok(Stmt::new(StmtKind::Module { name, body: None })))
            }
            Some(CanonicalKind::Equal) => {
                ctx.consume();
                let body = match ctx.parse_expr() {
                    Ok(expr) => expr,
                    Err(e) => return Some(Err(e)),
                };
                Some(Ok(Stmt::new(StmtKind::Module { name, body: Some(body) })))
            }
            Some(CanonicalKind::BlockOpen) => Some(Err(PlatformError::Plugin(
                "module block bodies are not supported yet".to_string(),
            ))),
            _ => Some(Err(PlatformError::Plugin(
                "expected ';' or '=' after module name".to_string(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_ast_canonical::Expr;
    use handl_expr::ModulePipeline;
    use handl_plugin_delimiters::Delimiters;
    use handl_plugin_identifiers::Identifiers;
    use handl_plugin_numbers::Numbers;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_values::PrimaryValues;
    use handl_vm_values_json::NonFunctionValue;

    fn pipeline() -> ModulePipeline {
        let mut p = handl_expr::ExpressionPipeline::new();
        p.register_token_rule(Box::new(Numbers::new()));
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));

        let mut m = ModulePipeline::new(p);
        m.register_statement(Box::new(Module::new()));
        m
    }

    #[test]
    fn parses_module_declaration() {
        let m = pipeline();
        let stmts = m.parse_module("module m;").unwrap();
        assert_eq!(stmts.len(), 1);
        assert_eq!(
            stmts[0],
            Stmt::new(StmtKind::Module {
                name: "m".to_string(),
                body: None,
            })
        );
    }

    #[test]
    fn parses_module_with_value() {
        let m = pipeline();
        let stmts = m.parse_module("module m = 1").unwrap();
        assert_eq!(stmts.len(), 1);
        assert_eq!(
            stmts[0],
            Stmt::new(StmtKind::Module {
                name: "m".to_string(),
                body: Some(Expr::Value(NonFunctionValue::new(1.0))),
            })
        );
    }
}
