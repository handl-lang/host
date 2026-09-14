//! Pretty-print `JsAst` to JavaScript source.

use handl_language::Span;
use handl_plugin_compile::CompileError;
use serde_json::Value;

use crate::lower::JsAst;

pub mod source_map;
pub use source_map::{Mapping, SourceMapBuilder};

/// Scans generated JavaScript for forbidden call patterns.
#[derive(Debug, Default, Clone, Copy)]
pub struct SecurityScanner;

impl SecurityScanner {
    /// Create a new scanner.
    pub fn new() -> Self {
        Self
    }

    /// Scan `js` for dangerous substrings.
    pub fn scan(&self, js: &str) -> Result<(), CompileError> {
        if js.contains("eval(") || js.contains("new Function(") || js.contains("Function(") {
            return Err(CompileError::Emit(
                "generated JS contains a dangerous call pattern".to_string(),
            ));
        }
        Ok(())
    }
}

fn json_string(value: &Value) -> Result<String, CompileError> {
    serde_json::to_string(value).map_err(|e| CompileError::Emit(e.to_string()))
}

/// Pretty-printer for [`JsAst`].
#[derive(Debug, Default, Clone, Copy)]
pub struct JsEmitter;

impl JsEmitter {
    /// Emit a `JsAst` as an expression string.
    pub fn emit_expr(&self, ast: &JsAst) -> Result<String, CompileError> {
        let mut out = String::new();
        self.emit_expr_to(ast, &mut out)?;
        Ok(out)
    }

    /// Emit a `JsAst` as a program string.
    pub fn emit_program(&self, ast: &JsAst) -> Result<String, CompileError> {
        let mut out = String::new();
        self.emit_stmt_to(ast, &mut out, 0)?;
        Ok(out)
    }

    /// Emit a `JsAst` and a Source Map V3 string.
    ///
    /// The source map records a root mapping from the start of the generated
    /// output to `span` in `source`.
    pub fn emit_with_source_map(
        &self,
        ast: &JsAst,
        source: &str,
        source_name: &str,
        span: Option<Span>,
    ) -> Result<(String, String), CompileError> {
        let js = self.emit_expr(ast)?;
        let mut builder = SourceMapBuilder::new(source_name, source, "main.js");
        let (source_line, source_col) = span
            .map(|s| source_map::span_to_line_col(source, s))
            .unwrap_or((0, 0));
        builder.add_mapping(0, 0, source_line, source_col, None);
        let map = builder.build();
        Ok((js, map))
    }

    /// Emit `ast` and run a security scan on the generated string.
    pub fn emit_secure(&self, ast: &JsAst) -> Result<String, CompileError> {
        let js = self.emit_expr(ast)?;
        SecurityScanner::new().scan(&js)?;
        Ok(js)
    }

    fn emit_expr_to(&self, ast: &JsAst, out: &mut String) -> Result<(), CompileError> {
        match ast {
            JsAst::Null => out.push_str("null"),
            JsAst::Bool(b) => out.push_str(&json_string(&Value::Bool(*b))?),
            JsAst::Num(n) => out.push_str(&json_string(&Value::Number(n.clone()))?),
            JsAst::Str(s) => out.push_str(&json_string(&Value::String(s.clone()))?),
            JsAst::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.emit_expr_to(item, out)?;
                }
                out.push(']');
            }
            JsAst::Object(props) => {
                out.push('{');
                for (i, (k, v)) in props.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&json_string(&Value::String(k.clone()))?);
                    out.push_str(": ");
                    self.emit_expr_to(v, out)?;
                }
                out.push('}');
            }
            JsAst::Ident(s) => out.push_str(s),
            JsAst::Member { obj, prop } => {
                if self.needs_parens(obj) {
                    out.push('(');
                    self.emit_expr_to(obj, out)?;
                    out.push(')');
                } else {
                    self.emit_expr_to(obj, out)?;
                }
                out.push('.');
                out.push_str(prop);
            }
            JsAst::Index { obj, key } => {
                if self.needs_parens(obj) {
                    out.push('(');
                    self.emit_expr_to(obj, out)?;
                    out.push(')');
                } else {
                    self.emit_expr_to(obj, out)?;
                }
                out.push('[');
                self.emit_expr_to(key, out)?;
                out.push(']');
            }
            JsAst::Call { callee, args } => {
                if self.call_needs_parens(callee) {
                    out.push('(');
                    self.emit_expr_to(callee, out)?;
                    out.push(')');
                } else {
                    self.emit_expr_to(callee, out)?;
                }
                out.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.emit_expr_to(arg, out)?;
                }
                out.push(')');
            }
            JsAst::ArrowFn { params, body } => self.emit_arrow_fn(params, body, out)?,
            JsAst::BinOp { op, left, right } => {
                out.push('(');
                self.emit_expr_to(left, out)?;
                out.push_str(&format!(" {} ", op.as_str()));
                self.emit_expr_to(right, out)?;
                out.push(')');
            }
            JsAst::UnaryOp { op, expr } => {
                out.push('(');
                out.push_str(op.as_str());
                self.emit_expr_to(expr, out)?;
                out.push(')');
            }
            JsAst::Assign { name, value } => {
                out.push_str(name);
                out.push_str(" = ");
                self.emit_expr_to(value, out)?;
            }
            JsAst::Raw(s) => out.push_str(s),
            // Statement forms should not appear as bare expressions.
            JsAst::Let { .. }
            | JsAst::ConstDecl { .. }
            | JsAst::Return(_)
            | JsAst::If { .. }
            | JsAst::While { .. }
            | JsAst::For { .. }
            | JsAst::Block(_) => self.emit_stmt_to(ast, out, 0)?,
        }
        Ok(())
    }

    fn emit_stmt_to(
        &self,
        ast: &JsAst,
        out: &mut String,
        indent: usize,
    ) -> Result<(), CompileError> {
        match ast {
            JsAst::Let { name, value } => {
                self.write_indent(out, indent);
                out.push_str(&format!("let {name} = "));
                self.emit_expr_to(value, out)?;
                out.push_str(";\n");
            }
            JsAst::ConstDecl { name, value } => {
                self.write_indent(out, indent);
                out.push_str(&format!("const {name} = "));
                self.emit_expr_to(value, out)?;
                out.push_str(";\n");
            }
            JsAst::Assign { name, value } => {
                self.write_indent(out, indent);
                out.push_str(&format!("{name} = "));
                self.emit_expr_to(value, out)?;
                out.push_str(";\n");
            }
            JsAst::Return(expr) => {
                self.write_indent(out, indent);
                out.push_str("return ");
                self.emit_expr_to(expr, out)?;
                out.push_str(";\n");
            }
            JsAst::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.write_indent(out, indent);
                out.push_str("if (");
                self.emit_expr_to(cond, out)?;
                out.push_str(") ");
                self.emit_branch(then_branch, out, indent)?;
                if let Some(else_b) = else_branch {
                    out.push_str(" else ");
                    self.emit_branch(else_b, out, indent)?;
                }
                out.push('\n');
            }
            JsAst::While { cond, body } => {
                self.write_indent(out, indent);
                out.push_str("while (");
                self.emit_expr_to(cond, out)?;
                out.push_str(") ");
                self.emit_branch(body, out, indent)?;
                out.push('\n');
            }
            JsAst::For {
                init,
                cond,
                update,
                body,
            } => {
                self.write_indent(out, indent);
                out.push_str("for (");
                self.emit_for_init(init, out)?;
                out.push_str("; ");
                self.emit_expr_to(cond, out)?;
                out.push_str("; ");
                self.emit_expr_to(update, out)?;
                out.push_str(") ");
                self.emit_branch(body, out, indent)?;
                out.push('\n');
            }
            JsAst::Block(stmts) => {
                self.write_indent(out, indent);
                out.push_str("{\n");
                for stmt in stmts {
                    self.emit_stmt_to(stmt, out, indent + 2)?;
                }
                self.write_indent(out, indent);
                out.push('}');
            }
            _ => {
                self.write_indent(out, indent);
                self.emit_expr_to(ast, out)?;
                out.push_str(";\n");
            }
        }
        Ok(())
    }

    fn emit_for_init(&self, ast: &JsAst, out: &mut String) -> Result<(), CompileError> {
        match ast {
            JsAst::Let { name, value } => {
                out.push_str(&format!("let {name} = "));
                self.emit_expr_to(value, out)?;
            }
            JsAst::ConstDecl { name, value } => {
                out.push_str(&format!("const {name} = "));
                self.emit_expr_to(value, out)?;
            }
            _ => self.emit_expr_to(ast, out)?,
        }
        Ok(())
    }

    fn emit_branch(
        &self,
        branch: &JsAst,
        out: &mut String,
        indent: usize,
    ) -> Result<(), CompileError> {
        if matches!(branch, JsAst::Block(_) | JsAst::If { .. }) {
            self.emit_stmt_to(branch, out, indent)?;
        } else {
            out.push_str("{\n");
            self.emit_stmt_to(branch, out, indent + 2)?;
            self.write_indent(out, indent);
            out.push('}');
        }
        Ok(())
    }

    fn emit_arrow_fn(
        &self,
        params: &[String],
        body: &JsAst,
        out: &mut String,
    ) -> Result<(), CompileError> {
        if params.len() == 1 {
            out.push_str(&params[0]);
        } else {
            out.push('(');
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(p);
            }
            out.push(')');
        }
        out.push_str(" => ");

        if self.is_expression(body) {
            self.emit_expr_to(body, out)?;
        } else if matches!(body, JsAst::Block(_)) {
            self.emit_stmt_to(body, out, 0)?;
        } else {
            out.push_str("{\n");
            self.emit_stmt_to(body, out, 2)?;
            out.push('}');
        }
        Ok(())
    }

    fn is_expression(&self, ast: &JsAst) -> bool {
        !matches!(
            ast,
            JsAst::Let { .. }
                | JsAst::ConstDecl { .. }
                | JsAst::Assign { .. }
                | JsAst::Return(_)
                | JsAst::If { .. }
                | JsAst::While { .. }
                | JsAst::For { .. }
                | JsAst::Block(_)
        )
    }

    fn needs_parens(&self, ast: &JsAst) -> bool {
        !matches!(
            ast,
            JsAst::Null
                | JsAst::Bool(_)
                | JsAst::Num(_)
                | JsAst::Str(_)
                | JsAst::Ident(_)
                | JsAst::Member { .. }
                | JsAst::Index { .. }
                | JsAst::Call { .. }
                | JsAst::Array(_)
                | JsAst::Object(_)
        )
    }

    fn call_needs_parens(&self, ast: &JsAst) -> bool {
        matches!(
            ast,
            JsAst::ArrowFn { .. }
                | JsAst::BinOp { .. }
                | JsAst::UnaryOp { .. }
                | JsAst::Object(_)
                | JsAst::Array(_)
        )
    }

    fn write_indent(&self, out: &mut String, indent: usize) {
        for _ in 0..indent {
            out.push(' ');
        }
    }
}
