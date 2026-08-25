use handl_expr::{ParseContext, StatementFragment, Stmt, StmtKind};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Statement fragment for `export <value>`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Export;

impl Export {
    /// Creates a new `export` statement fragment.
    pub fn new() -> Self {
        Self
    }
}

impl StatementFragment for Export {
    fn try_statement(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Stmt, PlatformError>> {
        let token = ctx.peek().cloned()?;
        match &token.kind {
            CanonicalKind::Identifier(name) if name == "export" => {}
            _ => return None,
        }
        ctx.consume();

        let value = match ctx.parse_expr() {
            Ok(expr) => expr,
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(Stmt::new(StmtKind::Export { value })))
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
        m.register_statement(Box::new(Export::new()));
        m
    }

    #[test]
    fn parses_export_identifier() {
        let m = pipeline();
        let stmts = m.parse_module("export a;").unwrap();
        assert_eq!(stmts.len(), 1);
        assert_eq!(
            stmts[0],
            Stmt::new(StmtKind::Export {
                value: Expr::var("a"),
            })
        );
    }

    #[test]
    fn parses_export_number() {
        let m = pipeline();
        let stmts = m.parse_module("export 42").unwrap();
        assert_eq!(stmts.len(), 1);
        assert_eq!(
            stmts[0],
            Stmt::new(StmtKind::Export {
                value: Expr::Value(NonFunctionValue::new(42.0)),
            })
        );
    }
}
