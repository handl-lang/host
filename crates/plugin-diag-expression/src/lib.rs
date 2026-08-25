use handl_ast_canonical::Expr;
use handl_expr::{Diagnostic, DiagnosticLevel, Diagnostics};
use handl_kit_canonical::CanonicalKit;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_vm_values_json::NonFunctionValue;
use serde_json::{json, Value as JsonValue};

/// Diagnostics plugin that warns about redundant double negation (`not not ...`).
pub struct ExpressionDiagnostics {
    capabilities: Vec<CapabilityId>,
}

impl ExpressionDiagnostics {
    /// Creates a new expression diagnostics plugin.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("diag.expression")],
        }
    }

    fn visit(&self, expr: &Expr<NonFunctionValue>, out: &mut Vec<Diagnostic>) {
        if let Expr::Call { name, args, .. } = expr {
            if name == "bool.not" && args.len() == 1 {
                if let Expr::Call {
                    name: inner, args: inner_args, ..
                } = &args[0]
                {
                    if inner == "bool.not" && inner_args.len() == 1 {
                        out.push(Diagnostic {
                            level: DiagnosticLevel::Warning,
                            message: "double negation: `not not <expr>` is redundant".to_string(),
                            span: None,
                        });
                    }
                }
            }
        }
        self.visit_children(expr, out);
    }

    fn visit_children(&self, expr: &Expr<NonFunctionValue>, out: &mut Vec<Diagnostic>) {
        match expr {
            Expr::Value(_) | Expr::Var(_) => {}
            Expr::Assign { value, .. } => self.visit(value, out),
            Expr::Function { params, ret, .. } => {
                for (_, ty) in params {
                    if let Some(t) = ty {
                        self.visit(t, out);
                    }
                }
                if let Some(r) = ret {
                    self.visit(r, out);
                }
            }
            Expr::Call { args, .. } => {
                for a in args {
                    self.visit(a, out);
                }
            }
            Expr::Apply { target, args } => {
                self.visit(target, out);
                for a in args {
                    self.visit(a, out);
                }
            }
            Expr::Handle { body, handlers } => {
                self.visit(body, out);
                for h in handlers {
                    self.visit(&h.body, out);
                }
            }
            Expr::Array(elements) => {
                for e in elements {
                    self.visit(e, out);
                }
            }
            Expr::Object(entries) => {
                for (_, e) in entries {
                    self.visit(e, out);
                }
            }
            Expr::Index { target, index } => {
                self.visit(target, out);
                self.visit(index, out);
            }
            Expr::FieldAccess { target, .. } => self.visit(target, out),
        }
    }
}

impl Default for ExpressionDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl Diagnostics for ExpressionDiagnostics {
    fn diagnose(&self, expr: &Expr<NonFunctionValue>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        self.visit(expr, &mut out);
        out
    }
}

impl Plugin for ExpressionDiagnostics {
    fn name(&self) -> &str {
        "handl-plugin-diag-expression"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        Some(json!({ "description": "lint canonical handl expressions" }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let source = match &request.payload {
            JsonValue::String(s) => s.as_str(),
            JsonValue::Object(o) => match o.get("source") {
                Some(JsonValue::String(s)) => s.as_str(),
                _ => {
                    return Err(PlatformError::Plugin(
                        "expected source string or {\"source\": ...}".to_string(),
                    ))
                }
            },
            _ => {
                return Err(PlatformError::Plugin(
                    "expected source string or {\"source\": ...}".to_string(),
                ))
            }
        };
        let kit = CanonicalKit::new();
        let expr = kit.parse(source)?;
        let diags = <Self as Diagnostics>::diagnose(self, &expr);
        let list = diags
            .iter()
            .map(|d| {
                json!({
                    "level": match d.level {
                        DiagnosticLevel::Error => "error",
                        DiagnosticLevel::Warning => "warning",
                        DiagnosticLevel::Info => "info",
                    },
                    "message": d.message,
                    "span": d.span.map(|s| json!({ "start": s.start, "end": s.end })),
                })
            })
            .collect::<Vec<_>>();
        Ok(CapabilityResponse(json!({ "diagnostics": list })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::Diagnostics;

    #[test]
    fn warns_on_double_negation() {
        let kit = CanonicalKit::new();
        let expr = kit.parse("not not true").unwrap();
        let diags = ExpressionDiagnostics::new().diagnose(&expr);
        assert!(!diags.is_empty());
        assert!(diags.iter().any(|d| d.level == DiagnosticLevel::Warning));
        assert!(diags.iter().any(|d| d.message.contains("double negation")));
    }

    #[test]
    fn no_warning_on_single_negation() {
        let kit = CanonicalKit::new();
        let expr = kit.parse("not true").unwrap();
        let diags = ExpressionDiagnostics::new().diagnose(&expr);
        assert!(diags.is_empty());
    }

    #[test]
    fn plugin_reports_capability() {
        let p = ExpressionDiagnostics::new();
        assert!(p.capabilities().iter().any(|c| c.as_str() == "diag.expression"));
    }
}
