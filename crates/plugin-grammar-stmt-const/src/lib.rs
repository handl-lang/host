use handl_expr::{expr_to_pattern, ParseContext, StatementFragment, Stmt, StmtKind};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Statement fragment for `const <pattern> = <expr>`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Const;

impl Const {
    /// Creates a new `const` statement fragment.
    pub fn new() -> Self {
        Self
    }
}

impl StatementFragment for Const {
    fn try_statement(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Stmt, PlatformError>> {
        let token = ctx.peek().cloned()?;
        match &token.kind {
            CanonicalKind::Identifier(name) if name == "const" => {}
            _ => return None,
        }
        ctx.consume();

        let pattern = match ctx.parse_primary() {
            Ok(expr) => match expr_to_pattern(&expr) {
                Ok(p) => p,
                Err(e) => return Some(Err(e)),
            },
            Err(e) => return Some(Err(e)),
        };

        if let Err(e) = ctx.expect(CanonicalKind::Equal) {
            return Some(Err(e));
        }

        let value = match ctx.parse_expr() {
            Ok(expr) => expr,
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(Stmt::new(StmtKind::Const { pattern, value })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_ast_canonical::{Expr, Pattern};
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
        m.register_statement(Box::new(Const::new()));
        m
    }

    #[test]
    fn parses_const() {
        let m = pipeline();
        let stmts = m.parse_module("const b = 2;").unwrap();
        assert_eq!(stmts.len(), 1);
        assert_eq!(
            stmts[0],
            Stmt::new(StmtKind::Const {
                pattern: Pattern::Identifier("b".to_string()),
                value: Expr::Value(NonFunctionValue::new(2.0)),
            })
        );
    }

    #[test]
    fn ignores_let() {
        let m = pipeline();
        assert!(m.parse_module("let a = 1").is_err());
    }
}
