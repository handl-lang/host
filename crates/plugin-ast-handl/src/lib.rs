//! AST composition layer for the handl language.
//!
//! This crate owns the unified [`KitNode`] enum, its [`Node`], [`Expr`], and
//! [`Stmt`] trait impls, and the `ast_to_kitnode` converter that turns generic
//! parser-produced [`AstNode`]s into the kit's concrete node tree.

use handl_plugin_ast::{Expr as ExprTrait, Node, Stmt as StmtTrait};
use handl_plugin_ast_apply::Apply;
use handl_plugin_ast_array::Array;
use handl_plugin_ast_assign::Assign;
use handl_plugin_ast_call::Call;
use handl_plugin_ast_function::Function;
use handl_plugin_ast_handle::{Handle, HandlerArm};
use handl_plugin_ast_handl_value::KitValue;
use handl_plugin_ast_index::Index;
use handl_plugin_ast_let::Let;
use handl_plugin_ast_match::{Match, MatchArm};
use handl_plugin_ast_object::Object;
use handl_plugin_ast_pipe::Pipe;
use handl_plugin_ast_stmt_export::Export;
use handl_plugin_ast_stmt_import::Import;
use handl_plugin_ast_stmt_module::Module;
use handl_plugin_ast_var::Var;
use handl_platform::{AstNode, PlatformError};

/// A generic metadata value for kit nodes.
pub type KitMetadata = ();

static UNIT: KitMetadata = ();

/// Generic AST node produced by the canonical kit.
#[derive(Debug, Clone, PartialEq)]
pub enum KitNode {
    /// A literal value.
    Literal(KitValue),
    /// A variable reference.
    Var(Var<KitNode, KitMetadata>),
    /// A named function call.
    Call(Call<KitNode, KitMetadata>),
    /// A function-value application.
    Apply(Box<Apply<KitNode, KitMetadata>>),
    /// A postfix index.
    Index(Box<Index<KitNode, KitMetadata>>),
    /// A pipe expression.
    Pipe(Box<Pipe<KitNode, KitMetadata>>),
    /// An array literal.
    Array(Array<KitNode, KitMetadata>),
    /// An object literal.
    Object(Object<KitNode, KitMetadata>),
    /// A `let` binding.
    Let(Box<Let<KitNode, KitMetadata>>),
    /// An assignment expression.
    Assign(Box<Assign<KitNode, KitMetadata>>),
    /// A `match` expression.
    Match(Box<Match<KitNode, KitMetadata>>),
    /// A `handle` expression.
    Handle(Box<Handle<KitNode, KitMetadata>>),
    /// A function literal: `(params) -> body`.
    Function(Box<Function<KitNode, KitMetadata>>),
    /// A statement block.
    Block(Array<KitNode, KitMetadata>),
    /// A `const` binding (reuses the `Let` shape: name + value).
    Const(Box<Let<KitNode, KitMetadata>>),
    /// An `import` statement.
    Import(Box<Import<KitNode, KitMetadata>>),
    /// An `export` statement.
    Export(Box<Export<KitNode, KitMetadata>>),
    /// A `module` declaration.
    Module(Box<Module<KitNode, KitMetadata>>),
}

impl Node for KitNode {
    type Value = KitValue;
    type Metadata = KitMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &UNIT
    }

    fn with_metadata(self, _metadata: Self::Metadata) -> Self
    where
        Self: Sized,
    {
        self
    }
}

impl ExprTrait for KitNode {
    fn value(value: Self::Value) -> Self
    where
        Self: Sized,
    {
        KitNode::Literal(value)
    }

    fn call<S>(name: S, args: Vec<Self>) -> Self
    where
        Self: Sized,
        S: Into<String>,
    {
        KitNode::Call(Call::new(name, args))
    }

    fn apply(target: Self, args: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        if let KitNode::Var(v) = &target {
            KitNode::Call(Call::new(&v.name, args))
        } else {
            KitNode::Apply(Box::new(Apply::new(target, args)))
        }
    }

    fn array(elements: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        KitNode::Array(Array::new(elements))
    }

    fn object(entries: Vec<(String, Self)>) -> Self
    where
        Self: Sized,
    {
        KitNode::Object(Object::new(entries))
    }

    fn index(target: Self, index: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Index(Box::new(Index::new(target, index)))
    }

    fn pipe(left: Self, right: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Pipe(Box::new(Pipe::new(left, right)))
    }

    fn assign(target: Self, value: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Assign(Box::new(Assign::new(target, value)))
    }

    fn match_(subject: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Match(Box::new(Match::new(subject)))
    }

    fn handle(expr: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Handle(Box::new(Handle::new(expr)))
    }
}

impl StmtTrait for KitNode {
    type Expr = KitNode;

    fn expression(expr: Self::Expr) -> Self
    where
        Self: Sized,
    {
        expr
    }

    fn let_<S>(name: S, value: Self::Expr) -> Self
    where
        Self: Sized,
        S: Into<String>,
    {
        KitNode::Let(Box::new(Let::new(name.into(), value)))
    }

    fn block(stmts: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        KitNode::Block(Array::new(stmts))
    }
}

/// Converts a generic `AstNode` produced by the parser into the kit's `KitNode`.
pub fn ast_to_kitnode(ast: &AstNode) -> Result<KitNode, PlatformError> {
    AST_CONVERTERS
        .iter()
        .find(|(kind, _)| *kind == ast.kind)
        .ok_or_else(|| {
            PlatformError::Plugin(format!("unsupported AST node kind '{}'", ast.kind))
        })
        .and_then(|(_, f)| f(ast))
}

type AstConverter = fn(&AstNode) -> Result<KitNode, PlatformError>;

const AST_CONVERTERS: &[(&str, AstConverter)] = &[
    ("value", convert_value),
    ("var", convert_var),
    ("call", convert_call),
    ("index", convert_index),
    ("pipe", convert_pipe),
    ("let", convert_let),
    ("assign", convert_assign),
    ("array", convert_array),
    ("object", convert_object),
    ("match", convert_match),
    ("handle", convert_handle),
    ("function", convert_function),
    ("stmt.let", convert_stmt_let),
    ("stmt.const", convert_stmt_const),
    ("stmt.import", convert_stmt_import),
    ("stmt.export", convert_stmt_export),
    ("stmt.module", convert_stmt_module),
];

fn convert_value(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let value = ast.payload.get("value").ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in AST payload".into())
    })?;

    let kit_value = if value.is_null() {
        KitValue::Null
    } else if let Some(n) = value.as_f64() {
        KitValue::Number(n)
    } else if let Some(s) = value.as_str() {
        KitValue::String(s.to_string())
    } else if let Some(b) = value.as_bool() {
        KitValue::Bool(b)
    } else {
        return Err(PlatformError::Plugin(format!(
            "unsupported primary value: {value}"
        )));
    };

    Ok(KitNode::Literal(kit_value))
}

fn convert_var(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in var AST payload".into())
        })?;

    Ok(KitNode::Var(Var::new(name.into())))
}

fn convert_call(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let callee = ast.payload.get("callee").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'callee' field in call AST payload".into())
    })?;
    let callee_ast: AstNode = serde_json::from_value(callee)
        .map_err(|e| PlatformError::Plugin(format!("invalid callee in call AST: {e}")))?;
    let callee = ast_to_kitnode(&callee_ast)?;

    let args = ast
        .payload
        .get("args")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'args' field in call AST payload".into())
        })?;
    let mut call_args = Vec::with_capacity(args.len());
    for arg in args {
        let arg_ast: AstNode = serde_json::from_value(arg.clone())
            .map_err(|e| PlatformError::Plugin(format!("invalid arg in call AST: {e}")))?;
        call_args.push(ast_to_kitnode(&arg_ast)?);
    }

    match callee {
        KitNode::Var(v) => Ok(KitNode::Call(Call::new(&v.name, call_args))),
        _ => Ok(KitNode::Apply(Box::new(Apply::new(callee, call_args)))),
    }
}

fn convert_index(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let target = ast.payload.get("target").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'target' field in index AST payload".into())
    })?;
    let target_ast: AstNode = serde_json::from_value(target)
        .map_err(|e| PlatformError::Plugin(format!("invalid target in index AST: {e}")))?;
    let target = ast_to_kitnode(&target_ast)?;

    let index = ast.payload.get("index").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'index' field in index AST payload".into())
    })?;
    let index_ast: AstNode = serde_json::from_value(index)
        .map_err(|e| PlatformError::Plugin(format!("invalid index in index AST: {e}")))?;
    let index = ast_to_kitnode(&index_ast)?;

    Ok(KitNode::Index(Box::new(Index::new(target, index))))
}

fn convert_pipe(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let left = ast.payload.get("left").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'left' field in pipe AST payload".into())
    })?;
    let left_ast: AstNode = serde_json::from_value(left)
        .map_err(|e| PlatformError::Plugin(format!("invalid left in pipe AST: {e}")))?;
    let left = ast_to_kitnode(&left_ast)?;

    let right = ast.payload.get("right").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'right' field in pipe AST payload".into())
    })?;
    let right_ast: AstNode = serde_json::from_value(right)
        .map_err(|e| PlatformError::Plugin(format!("invalid right in pipe AST: {e}")))?;
    let right = ast_to_kitnode(&right_ast)?;

    Ok(KitNode::Pipe(Box::new(Pipe::new(left, right))))
}

fn convert_let(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in let AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in let AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value)
        .map_err(|e| PlatformError::Plugin(format!("invalid value in let AST: {e}")))?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Let(Box::new(Let::new(name.into(), value))))
}

fn convert_assign(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let target = ast.payload.get("target").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'target' field in assign AST payload".into())
    })?;
    let target_ast: AstNode = serde_json::from_value(target)
        .map_err(|e| PlatformError::Plugin(format!("invalid target in assign AST: {e}")))?;
    let target = ast_to_kitnode(&target_ast)?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in assign AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value)
        .map_err(|e| PlatformError::Plugin(format!("invalid value in assign AST: {e}")))?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Assign(Box::new(Assign::new(target, value))))
}

fn convert_array(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let elements = ast
        .payload
        .get("elements")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'elements' field in array AST payload".into())
        })?;
    let mut elems = Vec::with_capacity(elements.len());
    for elem in elements {
        let elem_ast: AstNode = serde_json::from_value(elem.clone()).map_err(|e| {
            PlatformError::Plugin(format!("invalid element in array AST: {e}"))
        })?;
        elems.push(ast_to_kitnode(&elem_ast)?);
    }
    Ok(KitNode::Array(Array::new(elems)))
}

fn convert_object(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let entries = ast
        .payload
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'entries' field in object AST payload".into())
        })?;
    let mut pairs = Vec::with_capacity(entries.len());
    for entry in entries {
        let key = entry
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PlatformError::Plugin("missing 'key' in object entry".into()))?;
        let value_val = entry.get("value").cloned().ok_or_else(|| {
            PlatformError::Plugin("missing 'value' in object entry".into())
        })?;
        let value_ast: AstNode = serde_json::from_value(value_val).map_err(|e| {
            PlatformError::Plugin(format!("invalid value in object entry: {e}"))
        })?;
        pairs.push((key.to_string(), ast_to_kitnode(&value_ast)?));
    }
    Ok(KitNode::Object(Object::new(pairs)))
}

fn convert_match(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let subject = ast.payload.get("subject").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'subject' field in match AST payload".into())
    })?;
    let subject_ast: AstNode = serde_json::from_value(subject)
        .map_err(|e| PlatformError::Plugin(format!("invalid subject in match AST: {e}")))?;
    let subject = ast_to_kitnode(&subject_ast)?;

    let mut arms = Vec::new();
    if let Some(arms_arr) = ast.payload.get("arms").and_then(|v| v.as_array()) {
        for arm in arms_arr {
            let pattern_ast: AstNode = serde_json::from_value(
                arm.get("pattern").cloned().unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| PlatformError::Plugin(format!("invalid pattern in match arm: {e}")))?;
            let pattern = ast_to_kitnode(&pattern_ast)?;

            let guard = if let Some(g) = arm.get("guard") {
                if g.is_null() {
                    None
                } else {
                    let guard_ast: AstNode = serde_json::from_value(g.clone())
                        .map_err(|e| PlatformError::Plugin(format!("invalid guard in match arm: {e}")))?;
                    Some(ast_to_kitnode(&guard_ast)?)
                }
            } else {
                None
            };

            let body_ast: AstNode = serde_json::from_value(
                arm.get("body").cloned().unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| PlatformError::Plugin(format!("invalid body in match arm: {e}")))?;
            let body = ast_to_kitnode(&body_ast)?;

            arms.push(MatchArm {
                pattern,
                guard,
                body,
                metadata: (),
            });
        }
    }

    if arms.is_empty() {
        Ok(KitNode::Match(Box::new(Match::new(subject))))
    } else {
        Ok(KitNode::Match(Box::new(Match::with_arms(subject, arms))))
    }
}

fn convert_handle(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let expr = ast.payload.get("expr").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'expr' field in handle AST payload".into())
    })?;
    let expr_ast: AstNode = serde_json::from_value(expr)
        .map_err(|e| PlatformError::Plugin(format!("invalid expr in handle AST: {e}")))?;
    let expr = ast_to_kitnode(&expr_ast)?;

    let mut handlers = Vec::new();
    if let Some(handlers_arr) = ast.payload.get("handlers").and_then(|v| v.as_array()) {
        for h in handlers_arr {
            let op_name = h.get("op_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let params: Vec<String> = h.get("params")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let continuation_name = h.get("continuation_name").and_then(|v| v.as_str()).unwrap_or("resume").to_string();

            let body_ast: AstNode = serde_json::from_value(
                h.get("body").cloned().unwrap_or(serde_json::Value::Null),
            )
            .map_err(|e| PlatformError::Plugin(format!("invalid body in handle clause: {e}")))?;
            let body = ast_to_kitnode(&body_ast)?;

            handlers.push(HandlerArm {
                op_name,
                params,
                continuation_name,
                body,
                metadata: (),
            });
        }
    }

    if handlers.is_empty() {
        Ok(KitNode::Handle(Box::new(Handle::new(expr))))
    } else {
        Ok(KitNode::Handle(Box::new(Handle::with_handlers(expr, handlers))))
    }
}

fn convert_function(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let params = ast
        .payload
        .get("params")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'params' field in function AST payload".into())
        })?;
    let param_names: Vec<String> = params
        .iter()
        .filter_map(|p| p.as_str().map(|s| s.to_string()))
        .collect();

    let body = ast.payload.get("body").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'body' field in function AST payload".into())
    })?;
    let body_ast: AstNode =
        serde_json::from_value(body).map_err(|e| PlatformError::Plugin(format!("invalid body in function AST: {e}")))?;
    let body = ast_to_kitnode(&body_ast)?;

    Ok(KitNode::Function(Box::new(Function::new(param_names, body))))
}

fn convert_stmt_let(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in stmt.let AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in stmt.let AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value)
        .map_err(|e| PlatformError::Plugin(format!("invalid value in stmt.let AST: {e}")))?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Let(Box::new(Let::new(name.into(), value))))
}

fn convert_stmt_const(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in stmt.const AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in stmt.const AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value).map_err(|e| {
        PlatformError::Plugin(format!("invalid value in stmt.const AST: {e}"))
    })?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Const(Box::new(Let::new(name.into(), value))))
}

fn convert_stmt_import(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let source = ast.payload.get("source").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'source' field in stmt.import AST payload".into())
    })?;
    let source_ast: AstNode = serde_json::from_value(source).map_err(|e| {
        PlatformError::Plugin(format!("invalid source in stmt.import AST: {e}"))
    })?;
    let source = ast_to_kitnode(&source_ast)?;

    Ok(KitNode::Import(Box::new(Import::new(source))))
}

fn convert_stmt_export(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in stmt.export AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value).map_err(|e| {
        PlatformError::Plugin(format!("invalid value in stmt.export AST: {e}"))
    })?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Export(Box::new(Export::new(value))))
}

fn convert_stmt_module(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in stmt.module AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned();
    let value = match value {
        Some(v) => {
            let value_ast: AstNode = serde_json::from_value(v).map_err(|e| {
                PlatformError::Plugin(format!("invalid value in stmt.module AST: {e}"))
            })?;
            Some(ast_to_kitnode(&value_ast)?)
        }
        None => None,
    };

    Ok(KitNode::Module(Box::new(Module::new(name.into(), value))))
}
