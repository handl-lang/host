//! Handl-authored model adapter: run a `.hnl` module as an `EvalEngine`.
//!
//! This is the ADR-0103 prototype seam. A `.hnl` module exports a `run` arrow
//! with signature `(Term, Value, ModelCtx) -> Value`. The host serializes the
//! `Term` and a minimal `ModelCtx` record, calls the arrow, and returns the
//! resulting `Value`.

use std::rc::Rc;

use handl_language::{
    ArrSource, AstRef, LangError, Language, LanguageView, RewriteWitness, Side, Term, Value,
};
use handl_platform::{EvalContext, Module};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use serde_json::json;

use crate::HandlModule;

/// Convert a `Term` into a `Value` shaped for Handl code: an object with a
/// `__tag__` field and one field per variant payload.
pub fn term_to_value(term: &Term) -> Value {
    match term {
        Term::Arr { value } => json!({ "__tag__": "Arr", "value": value }),
        Term::Const { value } => json!({ "__tag__": "Const", "value": value }),
        Term::Op {
            name,
            args,
            source,
            lit,
        } => json!({
            "__tag__": "Op",
            "name": name,
            "args": args.iter().map(term_to_value).collect::<Vec<_>>(),
            "source": arr_source_to_value(source),
            "lit": lit.as_ref().cloned().unwrap_or(Value::Null)
        }),
        Term::Seq { fst, snd } => json!({
            "__tag__": "Seq",
            "fst": term_to_value(fst),
            "snd": term_to_value(snd)
        }),
        Term::First { inner } => json!({ "__tag__": "First", "inner": term_to_value(inner) }),
        Term::Choice { l, r } => {
            json!({ "__tag__": "Choice", "l": term_to_value(l), "r": term_to_value(r) })
        }
        Term::Loop { body } => json!({ "__tag__": "Loop", "body": term_to_value(body) }),
        Term::App { callee, args } => json!({
            "__tag__": "App",
            "callee": term_to_value(callee),
            "args": args.iter().map(term_to_value).collect::<Vec<_>>()
        }),
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => json!({
            "__tag__": "Iter",
            "fuel": term_to_value(fuel),
            "predicate": term_to_value(predicate),
            "step": term_to_value(step),
            "initial": term_to_value(initial)
        }),
        Term::Child { role } => json!({ "__tag__": "Child", "role": role }),
        Term::Handle {
            body,
            target,
            impl_,
        } => json!({
            "__tag__": "Handle",
            "body": term_to_value(body),
            "target": target,
            "impl_": term_to_value(impl_)
        }),
        Term::Rewrite {
            source,
            target,
            witness,
        } => json!({
            "__tag__": "Rewrite",
            "source": term_to_value(source),
            "target": term_to_value(target),
            "witness": rewrite_witness_to_value(witness)
        }),
        Term::Path {
            endpoints: (a, b),
            witness,
        } => json!({
            "__tag__": "Path",
            "from": term_to_value(a),
            "to": term_to_value(b),
            "witness": witness.as_ref().map(|w| term_to_value(w)).unwrap_or(Value::Null)
        }),
        Term::Equiv { a, b, path } => json!({
            "__tag__": "Equiv",
            "a": term_to_value(a),
            "b": term_to_value(b),
            "path": path.as_ref().map(|p| term_to_value(p)).unwrap_or(Value::Null)
        }),
        Term::CellId { arrow } => json!({ "__tag__": "CellId", "arrow": term_to_value(arrow) }),
        Term::Src { cell } => json!({ "__tag__": "Src", "cell": term_to_value(cell) }),
        Term::Tgt { cell } => json!({ "__tag__": "Tgt", "cell": term_to_value(cell) }),
        Term::CellCompV { first, second } => json!({
            "__tag__": "CellCompV",
            "first": term_to_value(first),
            "second": term_to_value(second)
        }),
        Term::Whisker { side, arrow, cell } => json!({
            "__tag__": "Whisker",
            "side": side_to_value(side),
            "arrow": term_to_value(arrow),
            "cell": term_to_value(cell)
        }),
        Term::RecordProj { record, field } => json!({
            "__tag__": "RecordProj",
            "record": term_to_value(record),
            "field": field
        }),
        Term::RecordWith {
            record,
            field,
            value,
        } => json!({
            "__tag__": "RecordWith",
            "record": term_to_value(record),
            "field": field,
            "value": term_to_value(value)
        }),
        Term::Tag { tag, value } => {
            json!({ "__tag__": "Tag", "tag": tag, "value": term_to_value(value) })
        }
        Term::Univalence { equiv } => {
            json!({ "__tag__": "Univalence", "equiv": term_to_value(equiv) })
        }
        Term::Glue { family, path } => {
            json!({ "__tag__": "Glue", "family": term_to_value(family), "path": term_to_value(path) })
        }
        Term::Fibrant { base, family } => {
            json!({ "__tag__": "Fibrant", "base": term_to_value(base), "family": term_to_value(family) })
        }
        Term::JElim { path, motive, base } => json!({
            "__tag__": "JElim",
            "path": term_to_value(path),
            "motive": term_to_value(motive),
            "base": term_to_value(base)
        }),
        Term::Refine { sort, predicate } => json!({
            "__tag__": "Refine",
            "sort": term_to_value(sort),
            "predicate": term_to_value(predicate)
        }),
        Term::Cobind { grade, body } => {
            json!({ "__tag__": "Cobind", "grade": term_to_value(grade), "body": term_to_value(body) })
        }
    }
}

fn arr_source_to_value(source: &ArrSource) -> Value {
    match source {
        ArrSource::Native => Value::String("Native".into()),
        ArrSource::Foreign(s) => json!({ "__tag__": "Foreign", "value": s }),
        ArrSource::Judgment => Value::String("Judgment".into()),
        ArrSource::DynamicForeign => Value::String("DynamicForeign".into()),
    }
}

fn rewrite_witness_to_value(witness: &RewriteWitness) -> Value {
    match witness {
        RewriteWitness::Named(name) => json!({ "__tag__": "Named", "value": name }),
        RewriteWitness::Compute => json!({ "__tag__": "Compute" }),
        RewriteWitness::HandlerHint => json!({ "__tag__": "HandlerHint" }),
        RewriteWitness::Cast => json!({ "__tag__": "Cast" }),
    }
}

fn side_to_value(side: &Side) -> Value {
    match side {
        Side::Left => Value::String("Left".into()),
        Side::Right => Value::String("Right".into()),
    }
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
        let term_value = term_to_value(term);
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
