//! Hughes arrow-dialect emitter for the `Term` lowerer.

use handl_language::{Term, Value};

/// Emitter from the arrow-calculus `Term` to a Hughes-style source string.
#[derive(Debug, Default, Clone, Copy)]
pub struct HughesEmitter;

impl HughesEmitter {
    /// Emit `term` as a `handl-hughes` source string.
    pub fn emit(&self, term: &Term) -> String {
        self.emit_arrow(term, &[])
    }

    /// Emit `term` as an arrow.
    fn emit_arrow(&self, term: &Term, params: &[String]) -> String {
        match term {
            Term::Arr { value, .. } | Term::Const { value } => value_to_string(value),
            Term::Op { name, args, lit, .. } => match name.as_str() {
                "id" => params.first().cloned().unwrap_or_else(|| "id".to_string()),
                "proj_child" => match lit.as_ref().and_then(Value::as_u64) {
                    Some(n) if (n as usize) < params.len() => params[n as usize].clone(),
                    Some(n) => format!(".{}", n),
                    None => ".proj".to_string(),
                },
                "add" | "mul" => self.emit_math_arrow(name, args, params),
                "product" if args.len() == 2 => {
                    let a = self.emit_arrow(&args[0], params);
                    let b = self.emit_arrow(&args[1], params);
                    format!("({} *** {})", a, b)
                }
                "fn.fanout_fn" if args.len() == 2 => {
                    let a = self.emit_arrow(&args[0], params);
                    let b = self.emit_arrow(&args[1], params);
                    format!("({} &&& {})", a, b)
                }
                "record" => self.emit_record_arrow(args, lit, params),
                "field_access" if args.len() == 1 => {
                    let target = self.emit_arrow(&args[0], params);
                    let field = match lit.as_ref() {
                        Some(v) => v.to_string(),
                        None => "?".to_string(),
                    };
                    format!("({}).{}", target, field)
                }
                _ => {
                    let count = needed_count(term);
                    if count == 0 {
                        self.emit_expr(term, params)
                    } else {
                        let ps = param_names(count);
                        format!("{} ~> {}", pattern(&ps), self.emit_expr(term, &ps))
                    }
                }
            },
            Term::Seq { fst, snd } => {
                let fst_s = self.emit_arrow(fst, params);
                let snd_s = self.emit_arrow(snd, params);
                format!("{} >>> {}", fst_s, snd_s)
            }
            _ => "<term>".to_string(),
        }
    }

    /// Emit `add` or `mul` as an arrow.
    fn emit_math_arrow(&self, op: &str, args: &[Term], params: &[String]) -> String {
        let (sym, arr) = if op == "add" {
            ("+", "add")
        } else {
            ("*", "mul")
        };

        if args.len() != 2 {
            return format!("{}({})", op, args.iter().map(|a| self.emit_expr(a, params)).collect::<Vec<_>>().join(", "));
        }

        let a = &args[0];
        let b = &args[1];

        let a_ref = is_input_ref(a);
        let b_ref = is_input_ref(b);
        let a_const = is_constant(a);
        let b_const = is_constant(b);

        // Section arrow: (input op const) or (const op input).
        if a_ref && b_const {
            let c = self.emit_arrow(b, params);
            return format!("({} {})", sym, c);
        }
        if a_const && b_ref {
            let c = self.emit_arrow(a, params);
            return format!("({} {})", c, sym);
        }

        // Binary op on the incoming pair: arr("...").
        if a_ref && b_ref {
            return format!("arr(\"{}\")", arr);
        }

        // Both arguments are constant: evaluate to a literal value arrow.
        if a_const && b_const {
            let av = self.emit_expr(a, params);
            let bv = self.emit_expr(b, params);
            return format!("({} {} {})", av, sym, bv);
        }

        // General case: fan out the two arrows and then apply the native op.
        let a_s = self.emit_arrow(a, params);
        let b_s = self.emit_arrow(b, params);
        format!("(({}) &&& ({})) >>> arr(\"{}\")", a_s, b_s, arr)
    }

    /// Emit a record arrow.
    fn emit_record_arrow(&self, args: &[Term], lit: &Option<Value>, _params: &[String]) -> String {
        let keys: Vec<String> = match lit.as_ref() {
            Some(Value::Array(ks)) if ks.len() == args.len() => ks
                .iter()
                .map(|k| k.as_str().unwrap_or("?").to_string())
                .collect(),
            _ => (0..args.len()).map(|i| format!("field{}", i)).collect(),
        };

        // If every field is a projection of a common arrow, abstract that arrow
        // and introduce a record-parameter `r`.
        if let Some(target) = common_field_access_target(args) {
            let indices: Vec<String> = args
                .iter()
                .map(|a| match a {
                    Term::Op { lit: Some(idx), .. } => idx.to_string(),
                    _ => "?".to_string(),
                })
                .collect();
            let r = "r";
            let fields: Vec<String> = keys
                .iter()
                .zip(indices.iter())
                .map(|(k, i)| format!("{}: {}.{}", k, r, i))
                .collect();

            if is_id(target) {
                // The record is already the next step in a pipeline.
                return format!("({} ~> {{ {} }})", r, fields.join(", "));
            }

            // Run the common arrow first, then project into a record.
            let t_s = self.emit_arrow(target, &[]);
            return format!("{} >>> ({} ~> {{ {} }})", t_s, r, fields.join(", "));
        }

        // Self-contained record of arrows.
        let fields: Vec<String> = keys
            .iter()
            .zip(args.iter())
            .map(|(k, v)| format!("{}: {}", k, self.emit_arrow(v, &[])))
            .collect();
        format!("{{ {} }}", fields.join(", "))
    }

    /// Emit `term` as a value expression.
    fn emit_expr(&self, term: &Term, params: &[String]) -> String {
        match term {
            Term::Arr { value, .. } | Term::Const { value } => value_to_string(value),
            Term::Op { name, args, lit, .. } => match name.as_str() {
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
            Term::Seq { .. } => format!("({})", self.emit_arrow(term, params)),
            _ => "<term>".to_string(),
        }
    }
}

/// Return the number of input parameters a `Term` references.
fn needed_count(term: &Term) -> usize {
    match term {
        Term::Arr { .. } | Term::Const { .. } => 0,
        Term::Op { name, args, lit, .. } => {
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
        Term::Seq { fst, .. } => needed_count(fst),
        _ => 0,
    }
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

fn is_constant(term: &Term) -> bool {
    matches!(term, Term::Arr { .. } | Term::Const { .. })
}

fn is_input_ref(term: &Term) -> bool {
    match term {
        Term::Op { name, args, .. } if name == "id" && args.is_empty() => true,
        Term::Op { name, lit: Some(_), .. } if name == "proj_child" => true,
        _ => false,
    }
}

fn is_id(term: &Term) -> bool {
    matches!(term, Term::Op { name, args, .. } if name == "id" && args.is_empty())
}

fn is_current_param_ref(term: &Term, params: &[String]) -> bool {
    if params.len() != 1 {
        return false;
    }
    match term {
        Term::Op { name, args, .. } if name == "id" && args.is_empty() => true,
        Term::Op {
            name,
            lit: Some(n),
            ..
        } if name == "proj_child" => n
            .as_u64()
            .map(|i| i == 0)
            .unwrap_or(false),
        _ => false,
    }
}

fn common_field_access_target<'a>(args: &'a [Term]) -> Option<&'a Term> {
    let mut target: Option<&'a Term> = None;
    for a in args {
        match a {
            Term::Op {
                name,
                args: fa_args,
                lit: Some(_),
                ..
            } if name == "field_access" && fa_args.len() == 1 => {
                if let Some(t) = target {
                    if t != &fa_args[0] {
                        return None;
                    }
                } else {
                    target = Some(&fa_args[0]);
                }
            }
            _ => return None,
        }
    }
    target
}
