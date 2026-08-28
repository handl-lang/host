use handl_expr::*;
use handl_plugin_fmt_trivia::TriviaFormatter;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_vm_values_json::NonFunctionValue;
use serde_json::{json, Value as JsonValue};

/// Formatter plugin that turns a sequence of canonical `Stmt` values back into
/// source text, preserving leading and trailing comments.
pub struct StatementFormatter {
    capabilities: Vec<CapabilityId>,
    expression_formatter: Box<dyn Formatter>,
    trivia_formatter: TriviaFormatter,
}

impl StatementFormatter {
    /// Creates a `StatementFormatter` from an expression `Formatter` and a
    /// `TriviaFormatter`.
    pub fn new(
        expression_formatter: Box<dyn Formatter>,
        trivia_formatter: TriviaFormatter,
    ) -> Self {
        Self {
            capabilities: vec![CapabilityId::from("fmt.statements")],
            expression_formatter,
            trivia_formatter,
        }
    }

    /// Formats a slice of `Stmt` values as a single source string.
    pub fn format(&self, stmts: &[Stmt<NonFunctionValue>]) -> Result<String, PlatformError> {
        let mut parts = Vec::new();
        for stmt in stmts {
            let mut s = self.trivia_formatter.format_comments(&stmt.leading);
            let body = self.format_stmt_kind(&stmt.kind)?;
            let trailing = self.trivia_formatter.format_comments(&stmt.trailing);

            if trailing.is_empty() {
                s.push_str(&body);
            } else {
                s.push_str(&body);
                s.push(' ');
                s.push_str(&trailing);
            }
            if !matches!(stmt.kind, StmtKind::Expr(_)) {
                s.push(';');
            }
            parts.push(s);
        }
        Ok(parts.join("\n"))
    }

    fn format_stmt_kind(
        &self,
        kind: &StmtKind<NonFunctionValue>,
    ) -> Result<String, PlatformError> {
        match kind {
            StmtKind::Expr(expr) => self.expression_formatter.format(expr),
            StmtKind::Let { pattern, value } => Ok(format!(
                "let {} = {}",
                self.format_pattern(pattern),
                self.expression_formatter.format(value)?
            )),
            StmtKind::Const { pattern, value } => Ok(format!(
                "const {} = {}",
                self.format_pattern(pattern),
                self.expression_formatter.format(value)?
            )),
            StmtKind::Import { source } => Ok(format!(
                "import {}",
                self.expression_formatter.format(source)?
            )),
            StmtKind::Export { value } => Ok(format!(
                "export {}",
                self.expression_formatter.format(value)?
            )),
            StmtKind::Module { name, body } => match body {
                Some(body) => Ok(format!(
                    "module {} = {}",
                    name,
                    self.expression_formatter.format(body)?
                )),
                None => Ok(format!("module {}", name)),
            },
        }
    }

    fn format_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Identifier(name) => name.clone(),
            Pattern::Array(elements) => {
                let inner = elements
                    .iter()
                    .map(|p| self.format_pattern(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", inner)
            }
            Pattern::Object(entries) => {
                let inner = entries
                    .iter()
                    .map(|(k, p)| format!("{}: {}", self.fmt_key(k), self.format_pattern(p)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {} }}", inner)
            }
            Pattern::Rest(name) => format!("...{}", name),
        }
    }

    fn fmt_key(&self, key: &str) -> String {
        if is_identifier(key) {
            key.to_string()
        } else {
            serde_json::to_string(&JsonValue::String(key.to_string()))
                .unwrap_or_else(|_| format!("\"{}\"", key))
        }
    }
}

impl Plugin for StatementFormatter {
    fn name(&self) -> &str {
        "handl-plugin-fmt-stmt"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        Some(json!({ "description": "format canonical handl statements" }))
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::Plugin(
            "fmt.statements is not exposed through the capability interface".to_string(),
        ))
    }
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_vm_values_json::NonFunctionValue;

    struct TestFormatter;

    impl Formatter for TestFormatter {
        fn format(&self, expr: &Expr<NonFunctionValue>) -> Result<String, PlatformError> {
            match expr {
                Expr::Value(v) => Ok(v
                    .as_number()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "value".to_string())),
                Expr::Var(name) => Ok(name.clone()),
                _ => Ok(format!("{:?}", expr)),
            }
        }
    }

    fn formatter() -> StatementFormatter {
        StatementFormatter::new(Box::new(TestFormatter), TriviaFormatter::new())
    }

    #[test]
    fn format_let() {
        let f = formatter();
        let stmt = Stmt::new(StmtKind::Let {
            pattern: Pattern::Identifier("a".to_string()),
            value: Expr::Value(NonFunctionValue::new(1.0)),
        });
        assert_eq!(f.format(&[stmt]).unwrap(), "let a = 1;");
    }

    #[test]
    fn format_let_with_leading_comment() {
        let f = formatter();
        let stmt = Stmt {
            kind: StmtKind::Let {
                pattern: Pattern::Identifier("a".to_string()),
                value: Expr::Value(NonFunctionValue::new(1.0)),
            },
            leading: vec![Token::new(
                CanonicalKind::Comment(" hello".to_string()),
                Span::new(0, 8),
            )],
            trailing: Vec::new(),
        };
        assert_eq!(f.format(&[stmt]).unwrap(), "// hello\nlet a = 1;");
    }

    #[test]
    fn format_let_with_trailing_comment() {
        let f = formatter();
        let stmt = Stmt {
            kind: StmtKind::Let {
                pattern: Pattern::Identifier("a".to_string()),
                value: Expr::Value(NonFunctionValue::new(1.0)),
            },
            leading: Vec::new(),
            trailing: vec![Token::new(
                CanonicalKind::Comment(" x ".to_string()),
                Span::new(0, 7),
            )],
        };
        assert_eq!(f.format(&[stmt]).unwrap(), "let a = 1 /* x */;");
    }

    #[test]
    fn plugin_reports_capability() {
        let p = StatementFormatter::new(Box::new(TestFormatter), TriviaFormatter::new());
        assert!(p.capabilities().iter().any(|c| c.as_str() == "fmt.statements"));
    }
}
