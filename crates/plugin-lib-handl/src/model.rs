//! Handl-authored model adapter: run a `.hnl` module as an `EvalEngine`.
//!
//! This is the ADR-0103 prototype seam. A `.hnl` module exports a `run` arrow
//! with signature `(Term, Value, ModelCtx) -> Value`. The host serializes the
//! `Term` and a minimal `ModelCtx` record, calls the arrow, and returns the
//! resulting `Value`.

use std::rc::Rc;

use handl_language::{
    AstRef, LangError, Language, LanguageView, RewriteWitness, Term, Value,
};
use handl_platform::{EvalContext, Module};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use plugin_engine::{Model, ModelCtx, RewriteResult, TermData};
use serde_json::json;

use crate::HandlModule;

/// Convert a `Term` into a `Value` shaped for Handl code: an object with a
/// `__tag__` field and one field per variant payload.
///
/// This delegates to [`TermData::from_term`], the canonical conversion shared
/// with `plugin-engine`, so the JSON shape stays identical to what
/// Handl-authored models consume.
pub fn term_to_value(term: &Term) -> Value {
    TermData::from_term(term).into_value()
}

/// A Handl-authored value model backed by a `.hnl` module's `run` arrow.
#[derive(Clone)]
pub struct HandlModel {
    module: Rc<HandlModule>,
}

impl HandlModel {
    /// Create a new model from a `HandlModule` that exports `run`.
    pub fn new(module: HandlModule) -> Self {
        Self {
            module: Rc::new(module),
        }
    }

    /// Invoke the module's `run(term, incoming, ctx)` arrow.
    pub fn run(
        &self,
        term: &Term,
        incoming: &Value,
        ctx_value: &Value,
    ) -> Result<Value, LangError> {
        self.run_data(&TermData::from_term(term), incoming, ctx_value)
    }

    /// Invoke the module's `run(term, incoming, ctx)` arrow with a [`TermData`]
    /// directly, skipping the `Term` → `Value` conversion.
    pub fn run_data(
        &self,
        term: &TermData,
        incoming: &Value,
        ctx_value: &Value,
    ) -> Result<Value, LangError> {
        let term_value = term.as_value().clone();
        let func = self
            .module
            .function("run")
            .ok_or_else(|| LangError::engine("HandlModel: module has no 'run' export"))?;
        let mut eval_ctx = EvalContext::new();
        let args = vec![term_value, incoming.clone(), ctx_value.clone()];
        func.call(&args, &mut eval_ctx)
            .map_err(|e| LangError::engine(format!("HandlModel::run: {e}")))
    }
}

/// An `EvalEngine` that delegates to a `HandlModel`.
///
/// Register this engine in an `EngineRegistry` under a name like `"handl-value"`
/// and select it via `RuntimeConfig::eval`.
#[derive(Clone)]
pub struct HandlModelEngine {
    model: HandlModel,
}

impl HandlModelEngine {
    /// Wrap a `HandlModule` (which exports `run`) as an `EvalEngine`.
    pub fn new(module: HandlModule) -> Self {
        Self {
            model: HandlModel::new(module),
        }
    }

    /// Wrap an existing `HandlModel`.
    pub fn from_model(model: HandlModel) -> Self {
        Self { model }
    }
}

impl EvalEngine for HandlModelEngine {
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError> {
        let ast = &req.ast;
        if ast.is_empty() {
            return Ok(Value::Null);
        }

        // Pratt convention: the root is the last node in the arena.
        let root_ref = AstRef::new(ast.len() - 1);
        let node = ast.at(root_ref)?;
        let term = match lang.eval_rule(&node.kind) {
            Some(scheme) => scheme,
            None => return Ok(node.payload.clone()),
        };

        let incoming = Value::Null;
        let ctx_value = json!({ "handler_stack": [], "node_ref": root_ref.0 });

        self.model.run(term, &incoming, &ctx_value)
    }

    fn box_clone(&self) -> Box<dyn EvalEngine> {
        Box::new(self.clone())
    }
}

/// Adapter wrapping a [`HandlModel`] as a [`Model`] with `Value = serde_json::Value`.
///
/// This is the ADR-0103 Phase 4 seam: a Handl-authored model (data) plugged
/// into the `Model` trait, interchangeable with the Rust `ValueModel`.
///
/// The `HandlModel` runs a `.hnl` module's `run(term, incoming, ctx)` arrow,
/// converting the `Term` to JSON via [`term_to_value`] and the `ModelCtx`
/// to a JSON context record. Results flow back as `serde_json::Value`.
///
/// Errors from the Handl model (`handl_language::LangError`) are collapsed to
/// `plugin_engine::LangError::NotImplemented`, matching the `ValueModel`
/// convention — the `plugin_engine::LangError` enum is intentionally minimal
/// (ADR-0103 TODO: give it a payload so real errors propagate).
#[derive(Clone)]
pub struct HandlModelAdapter {
    model: HandlModel,
}

impl HandlModelAdapter {
    /// Create a new adapter from a `HandlModule` that exports `run`.
    pub fn new(module: HandlModule) -> Self {
        Self {
            model: HandlModel::new(module),
        }
    }

    /// Wrap an existing `HandlModel`.
    pub fn from_model(model: HandlModel) -> Self {
        Self { model }
    }

    /// Run the model on a [`TermData`] directly, without going through `Term`.
    pub fn run_data(
        &self,
        term: &TermData,
        incoming: &Value,
        ctx: &ModelCtx,
    ) -> Result<Value, String> {
        let ctx_value = json!({
            "handler_stack": ctx.handler_stack,
            "node_ref": ctx.node_ref,
        });
        self.model
            .run_data(term, incoming, &ctx_value)
            .map_err(|e| e.to_string())
    }
}

impl Model for HandlModelAdapter {
    type Value = serde_json::Value;

    fn run(
        &self,
        term: &Term,
        incoming: serde_json::Value,
        ctx: &mut ModelCtx,
    ) -> Result<serde_json::Value, plugin_engine::LangError> {
        self.run_data(&TermData::from_term(term), &incoming, ctx)
            .map_err(|_| plugin_engine::LangError::NotImplemented)
    }

    fn rewrite(
        &self,
        _source: &Term,
        target: &Term,
        _witness: &RewriteWitness,
        incoming: &serde_json::Value,
        ctx: &mut ModelCtx,
    ) -> Result<RewriteResult<serde_json::Value>, plugin_engine::LangError> {
        // For now, just run the target (rewrites evaluate to target).
        let result = self.run(target, incoming.clone(), ctx)?;
        Ok(RewriteResult::Rewritten(result))
    }
}
