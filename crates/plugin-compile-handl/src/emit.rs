//! Hughes arrow-dialect emitter for the `Term` lowerer.

use handl_language::{Term, Value};

/// Emitter from the arrow-calculus `Term` to a Hughes-style source string.
#[derive(Debug, Default, Clone, Copy)]
pub struct HughesEmitter;

impl HughesEmitter {
    /// Emit `term` as a `handl-hughes` source string.
    ///
    /// The emitted dialect is canonical handl surface syntax (ADR-0079
    /// unified the arrow DSL onto the canonical language): `~>` lambdas,
    /// `>>>`/`***`/`&&&` combinators, `arr <op>`, `{k: v}` objects and
    /// `expr.i` field access — all forms the canonical parser and
    /// `lower_ast` accept, so the emitted text round-trips back to the
    /// same `Term`.
    pub fn emit(&self, term: &Term) -> String {
        self.emit_arrow(term, &[])
    }

    /// Emit `term` as an arrow.
    ///
    /// An `Op`'s arguments are *expressions over the incoming value* —
    /// `id` is the whole input and `proj_child n` is its n-th component —
    /// so an op that references the input is emitted as a lambda
    /// `(p0, …) ~> expr` whose parameter names stand in for those
    /// projections. Sub-arrows that ignore the input emit in expression
    /// position directly.
    fn emit_arrow(&self, term: &Term, params: &[String]) -> String {
        match term {
            Term::Arr { value, .. } => {
                if let Some(inner) = quoted_term(value) {
                    return self.emit_quoted_arrow(&inner);
                }
                value_to_string(value)
            }
            Term::Const { value } => value_to_string(value),
            Term::Op { name, .. } if name == "id" => {
                params.first().cloned().unwrap_or_else(|| "id".to_string())
            }
            Term::Op { .. } => {
                let count = needed_count(term);
                if count == 0 {
                    self.emit_expr(term, params)
                } else {
                    // `~>` binds looser than `>>>`/`***`/`&&&`, so the
                    // lambda needs parens wherever it appears.
                    let ps = param_names(count);
                    format!("({} ~> {})", pattern(&ps), self.emit_expr(term, &ps))
                }
            }
            Term::Seq { axis, fst, snd } => {
                let fst_s = self.emit_arrow(fst, params);
                let snd_s = self.emit_arrow(snd, params);
                // `>>>` is `>>>[0]`; emit the axis bracket only for
                // non-default axes (ADR-0120 Phase 2).
                if axis.is_default_axis() {
                    format!("{} >>> {}", fst_s, snd_s)
                } else {
                    format!("{} >>>[{}] {}", fst_s, self.emit_expr(axis, params), snd_s)
                }
            }
            _ => "<term>".to_string(),
        }
    }

    /// Emit a quoted arrow value (`Arr` whose payload serializes a `Term`)
    /// as a lambda `(p0, …) ~> body` — the surface form that lowers back
    /// to the same quoted `Arr`.
    fn emit_quoted_arrow(&self, inner: &Term) -> String {
        let count = needed_count(inner).max(1);
        let ps = param_names(count);
        format!("({} ~> {})", pattern(&ps), self.emit_expr(inner, &ps))
    }

    /// Emit `term` as a value expression.
    fn emit_expr(&self, term: &Term, params: &[String]) -> String {
        match term {
            Term::Arr { value, .. } => {
                if let Some(inner) = quoted_term(value) {
                    return self.emit_quoted_arrow(&inner);
                }
                value_to_string(value)
            }
            Term::Const { value } => value_to_string(value),
            Term::Op {
                name, args, lit, ..
            } => match name.as_str() {
                "id" => params.first().cloned().unwrap_or_else(|| "id".to_string()),
                "proj_child" => match lit.as_ref().and_then(Value::as_u64) {
                    Some(n) if (n as usize) < params.len() => params[n as usize].clone(),
                    Some(n) => format!(".{}", n),
                    None => ".proj".to_string(),
                },
                "add" if args.len() == 2 => format!(
                    "({} + {})",
                    self.emit_expr(&args[0], params),
                    self.emit_expr(&args[1], params)
                ),
                "sub" if args.len() == 2 => format!(
                    "({} - {})",
                    self.emit_expr(&args[0], params),
                    self.emit_expr(&args[1], params)
                ),
                "mul" if args.len() == 2 => format!(
                    "({} * {})",
                    self.emit_expr(&args[0], params),
                    self.emit_expr(&args[1], params)
                ),
                "div" if args.len() == 2 => format!(
                    "({} / {})",
                    self.emit_expr(&args[0], params),
                    self.emit_expr(&args[1], params)
                ),
                "product" if args.len() == 2 => format!(
                    "({}) *** ({})",
                    self.emit_expr(&args[0], params),
                    self.emit_expr(&args[1], params)
                ),
                "fn.fanout_fn" if args.len() == 2 => format!(
                    "({}) &&& ({})",
                    self.emit_expr(&args[0], params),
                    self.emit_expr(&args[1], params)
                ),
                "record" => {
                    let keys: Vec<String> = match lit.as_ref() {
                        Some(Value::Array(ks)) if ks.len() == args.len() => ks
                            .iter()
                            .map(|k| k.as_str().unwrap_or("?").to_string())
                            .collect(),
                        _ => (0..args.len()).map(|i| format!("field{}", i)).collect(),
                    };
                    let fields: Vec<String> = keys
                        .iter()
                        .zip(args.iter())
                        .map(|(k, v)| format!("{}: {}", k, self.emit_expr(v, params)))
                        .collect();
                    format!("{{ {} }}", fields.join(", "))
                }
                "field_access" if args.len() == 1 => {
                    let idx = match lit.as_ref() {
                        Some(v) => v.to_string(),
                        None => "?".to_string(),
                    };
                    if params.len() == 1 && is_current_param_ref(&args[0], params) {
                        format!("{}.{}", params[0], idx)
                    } else {
                        let target = self.emit_expr(&args[0], params);
                        format!("({}).{}", target, idx)
                    }
                }
                _ => {
                    let arg_s = args
                        .iter()
                        .map(|a| self.emit_expr(a, params))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}({})", name, arg_s)
                }
            },
            // `f(a)` — arrow application. A quoted callee emits as a
            // parenthesised lambda `(x ~> body)`; `lower_ast` quotes the
            // function and rebuilds the `App`, so the round-trip is exact.
            Term::App { callee, args } => {
                let arg_s = args
                    .iter()
                    .map(|a| self.emit_expr(a, params))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", self.emit_expr(callee, params), arg_s)
            }
            Term::Seq { .. } => format!("({})", self.emit_arrow(term, params)),
            _ => "<term>".to_string(),
        }
    }
}

/// Return the number of input parameters a `Term` references.
fn needed_count(term: &Term) -> usize {
    match term {
        Term::Arr { .. } | Term::Const { .. } => 0,
        Term::Op {
            name, args, lit, ..
        } => {
            if name == "id" {
                return 1;
            }
            if name == "proj_child" {
                if let Some(n) = lit.as_ref().and_then(Value::as_u64) {
                    return n as usize + 1;
                }
                return 0;
            }
            let mut max = 0;
            for a in args {
                if !matches!(a, Term::Seq { .. }) {
                    let c = needed_count(a);
                    if c > max {
                        max = c;
                    }
                }
            }
            max
        }
        Term::App { args, .. } => args.iter().map(needed_count).max().unwrap_or(0),
        Term::Seq { fst, .. } => needed_count(fst),
        _ => 0,
    }
}

/// If `value` is a quoted arrow — a serialized `Term` in `Arr.value`
/// (ADR-0120 Phase 1 quoting, the shape `lower_ast`'s `quote_term`
/// produces) — deserialize and return it.
fn quoted_term(value: &Value) -> Option<Term> {
    serde_json::from_value::<Term>(value.clone()).ok()
}

/// Generate parameter names for a given count.
fn param_names(count: usize) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec!["x".to_string()];
    }
    (0..count)
        .map(|i| match i {
            0 => "a".to_string(),
            1 => "b".to_string(),
            2 => "c".to_string(),
            3 => "d".to_string(),
            _ => format!("p{}", i),
        })
        .collect()
}

fn pattern(params: &[String]) -> String {
    if params.len() == 1 {
        params[0].clone()
    } else {
        format!("({})", params.join(", "))
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn is_current_param_ref(term: &Term, params: &[String]) -> bool {
    if params.len() != 1 {
        return false;
    }
    match term {
        Term::Op { name, args, .. } if name == "id" && args.is_empty() => true,
        Term::Op {
            name, lit: Some(n), ..
        } if name == "proj_child" => n.as_u64().map(|i| i == 0).unwrap_or(false),
        _ => false,
    }
}
