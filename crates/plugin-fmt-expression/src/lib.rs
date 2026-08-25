use handl_ast_canonical::{Expr, Pattern};
use handl_expr::Formatter;
use handl_kit_canonical::CanonicalKit;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_vm_values_json::NonFunctionValue;
use serde_json::{json, Value as JsonValue};

/// Formatter plugin that turns a canonical `Expr` tree back into source text.
pub struct ExpressionFormatter {
    capabilities: Vec<CapabilityId>,
}

impl ExpressionFormatter {
    /// Creates a new expression formatter.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("fmt.expression")],
        }
    }

    fn fmt(&self, expr: &Expr<NonFunctionValue>, min_bp: u32) -> String {
        let s = self.fmt_inner(expr);
        if self.lbp(expr) < min_bp {
            format!("({})", s)
        } else {
            s
        }
    }

    /// Returns the left-binding power of an expression. Higher binds tighter.
    fn lbp(&self, expr: &Expr<NonFunctionValue>) -> u32 {
        match expr {
            Expr::Call { name, .. } => match name.as_str() {
                "fn.pipe" | "fn.apply" => 40,
                "fn.flow" | "fn.compose" => 60,
                "if" => 30,
                "bool.not" | "type.typeof" => 80,
                "mood.int" | "mood.norm" | "mood.pot" | "mood.neg" | "mood.cnd" => 85,
                _ => 90,
            },
            Expr::Assign { .. } => 1,
            _ => 100,
        }
    }

    fn fmt_inner(&self, expr: &Expr<NonFunctionValue>) -> String {
        match expr {
            Expr::Value(v) => self.fmt_value(v),
            Expr::Var(name) => name.clone(),
            Expr::Assign { target, value } => match value.as_ref() {
                Expr::Function { .. } => self.fmt_named_function(target, value.as_ref()),
                _ => format!("{} = {}", self.fmt_pattern(target), self.fmt(value.as_ref(), 0)),
            },
            Expr::Function { .. } => self.fmt_function(expr),
            Expr::Call { name, args, type_args } => self.fmt_call(name, args, type_args),
            Expr::Apply { target, args } => {
                let t = self.fmt(target.as_ref(), 90);
                let a = args
                    .iter()
                    .map(|e| self.fmt(e, 0))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", t, a)
            }
            Expr::Handle { .. } => "/* handle */".to_string(),
            Expr::Array(elements) => {
                let e = elements
                    .iter()
                    .map(|e| self.fmt(e, 0))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", e)
            }
            Expr::Object(entries) => {
                let e = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", self.fmt_key(k), self.fmt(v, 0)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {} }}", e)
            }
            Expr::Index { target, index } => {
                format!("{}[{}]", self.fmt(target.as_ref(), 95), self.fmt(index.as_ref(), 0))
            }
            Expr::FieldAccess { target, field } => {
                format!("{}.{}", self.fmt(target.as_ref(), 95), field)
            }
        }
    }

    fn fmt_named_function(&self, target: &Pattern, value: &Expr<NonFunctionValue>) -> String {
        match target {
            Pattern::Identifier(name) => format!("{}{}", name, self.fmt_function(value)),
            _ => format!("{} = {}", self.fmt_pattern(target), self.fmt_function(value)),
        }
    }

    fn fmt_function(&self, expr: &Expr<NonFunctionValue>) -> String {
        match expr {
            Expr::Function {
                type_params,
                params,
                ret,
                body,
            } => {
                let mut s = String::new();
                if !type_params.is_empty() {
                    s.push('<');
                    s.push_str(&type_params.join(", "));
                    s.push('>');
                }
                let p = params
                    .iter()
                    .map(|(pat, ty)| {
                        let mut ps = self.fmt_pattern(pat);
                        if let Some(t) = ty {
                            ps.push_str(": ");
                            ps.push_str(&self.fmt(t.as_ref(), 0));
                        }
                        ps
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                s.push('(');
                s.push_str(&p);
                s.push(')');
                if let Some(r) = ret {
                    s.push_str(": ");
                    s.push_str(&self.fmt(r.as_ref(), 0));
                }
                s.push_str(" -> ");
                s.push_str(body);
                s
            }
            _ => self.fmt(expr, 100),
        }
    }

    fn fmt_call(
        &self,
        name: &str,
        args: &[Expr<NonFunctionValue>],
        type_args: &[String],
    ) -> String {
        match name {
            "fn.pipe" if args.len() == 2 => {
                let left = self.fmt(&args[0], 40);
                let right = self.fmt(&args[1], 41);
                format!("{} |> {}", left, right)
            }
            "fn.apply" if args.len() == 2 => {
                if let Expr::Array(arr) = &args[1] {
                    if let Some(arg) = arr.first() {
                        let left = self.fmt(arg, 39);
                        let right = self.fmt(&args[0], 40);
                        return format!("{} <| {}", left, right);
                    }
                }
                self.fmt_plain_call(name, args, type_args)
            }
            "fn.flow" | "fn.compose" if args.len() == 2 => {
                let op = if name == "fn.flow" { ">>>" } else { "<<<" };
                let left = self.fmt(&args[0], 60);
                let right = self.fmt(&args[1], 61);
                format!("{} {} {}", left, op, right)
            }
            "if" if args.len() == 3 => {
                let cond = self.fmt(&args[0], 30);
                let then_ = self.fmt(&args[1], 29);
                let else_ = self.fmt(&args[2], 29);
                format!("{} ? {} : {}", cond, then_, else_)
            }
            "bool.not" if args.len() == 1 => {
                format!("not {}", self.fmt(&args[0], 100))
            }
            "type.typeof" if args.len() == 1 => {
                format!("typeof {}", self.fmt(&args[0], 100))
            }
            "mood.int" | "mood.norm" | "mood.pot" | "mood.neg" | "mood.cnd"
                if args.len() >= 2 =>
            {
                let keyword = match name {
                    "mood.int" => "is",
                    "mood.norm" => "must",
                    "mood.pot" => "may",
                    "mood.neg" => "not",
                    "mood.cnd" => "when",
                    _ => unreachable!(),
                };
                let target = self.fmt(&args[0], 85);
                let kind = self.fmt_mood_kind(&args[1]);
                if name == "mood.cnd" && args.len() == 3 {
                    format!("{} {} {} else {}", target, keyword, kind, self.fmt(&args[2], 0))
                } else {
                    format!("{} {} {}", target, keyword, kind)
                }
            }
            _ => self.fmt_plain_call(name, args, type_args),
        }
    }

    fn fmt_plain_call(
        &self,
        name: &str,
        args: &[Expr<NonFunctionValue>],
        type_args: &[String],
    ) -> String {
        let mut s = name.to_string();
        if !type_args.is_empty() {
            s.push('<');
            s.push_str(&type_args.join(", "));
            s.push('>');
        }
        let a = args
            .iter()
            .map(|e| self.fmt(e, 0))
            .collect::<Vec<_>>()
            .join(", ");
        s.push('(');
        s.push_str(&a);
        s.push(')');
        s
    }

    fn fmt_mood_kind(&self, expr: &Expr<NonFunctionValue>) -> String {
        match expr {
            Expr::Value(v) => match v.as_str() {
                Some(s) => s.to_string(),
                _ => self.fmt_value(v),
            },
            _ => self.fmt(expr, 100),
        }
    }

    fn fmt_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Identifier(name) => name.clone(),
            Pattern::Array(elements) => {
                let e = elements
                    .iter()
                    .map(|p| self.fmt_pattern(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", e)
            }
            Pattern::Object(entries) => {
                let e = entries
                    .iter()
                    .map(|(k, p)| format!("{}: {}", self.fmt_key(k), self.fmt_pattern(p)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {} }}", e)
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

    fn fmt_value(&self, v: &NonFunctionValue) -> String {
        self.fmt_json_value(v.inner())
    }

    fn fmt_json_value(&self, v: &JsonValue) -> String {
        match v {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(b) => b.to_string(),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    i.to_string()
                } else if let Some(u) = n.as_u64() {
                    u.to_string()
                } else {
                    let f = n.as_f64().unwrap_or(0.0);
                    if f.is_finite() && f.fract() == 0.0 && f.abs() < (i64::MAX as f64) {
                        (f as i64).to_string()
                    } else {
                        format!("{}", f)
                    }
                }
            }
            JsonValue::String(s) => serde_json::to_string(&JsonValue::String(s.clone()))
                .unwrap_or_else(|_| format!("\"{}\"", s)),
            JsonValue::Array(arr) => {
                let e = arr
                    .iter()
                    .map(|x| self.fmt_json_value(x))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", e)
            }
            JsonValue::Object(obj) => {
                let e = obj
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}: {}",
                            serde_json::to_string(&JsonValue::String(k.clone()))
                                .unwrap_or_else(|_| format!("\"{}\"", k)),
                            self.fmt_json_value(v)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {} }}", e)
            }
        }
    }
}

impl Default for ExpressionFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for ExpressionFormatter {
    fn format(&self, expr: &Expr<NonFunctionValue>) -> Result<String, PlatformError> {
        Ok(self.fmt(expr, 0))
    }
}

impl Plugin for ExpressionFormatter {
    fn name(&self) -> &str {
        "handl-plugin-fmt-expression"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        Some(json!({ "description": "format canonical handl expressions" }))
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
        let formatted = <Self as Formatter>::format(self, &expr)?;
        Ok(CapabilityResponse(json!({ "formatted": formatted })))
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
    use handl_ast_canonical::Expr;
    use handl_expr::Formatter as Fmt;
    use handl_vm_values_json::NonFunctionValue;

    #[test]
    fn pipe_round_trip() {
        let f = ExpressionFormatter::new();
        let expr = Expr::call(
            "fn.pipe",
            vec![
                Expr::Value(NonFunctionValue::new(1.0)),
                Expr::var("add"),
            ],
        );
        let s = f.format(&expr).unwrap();
        assert_eq!(s, "1 |> add");
    }

    #[test]
    fn format_basic_values() {
        let f = ExpressionFormatter::new();
        assert_eq!(f.format(&Expr::Value(NonFunctionValue::new(1.0))).unwrap(), "1");
        assert_eq!(f.format(&Expr::Value(NonFunctionValue::new(true))).unwrap(), "true");
        assert_eq!(f.format(&Expr::Value(NonFunctionValue::new(false))).unwrap(), "false");
        assert_eq!(f.format(&Expr::Value(NonFunctionValue::new(serde_json::Value::Null))).unwrap(), "null");
        assert_eq!(f.format(&Expr::var("x")).unwrap(), "x");
    }

    #[test]
    fn format_not() {
        let f = ExpressionFormatter::new();
        let expr = Expr::call("bool.not", vec![Expr::Value(NonFunctionValue::new(true))]);
        assert_eq!(f.format(&expr).unwrap(), "not true");
    }

    #[test]
    fn plugin_reports_capability() {
        let p = ExpressionFormatter::new();
        assert!(p.capabilities().iter().any(|c| c.as_str() == "fmt.expression"));
    }
}
