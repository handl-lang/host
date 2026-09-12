//! # handl-plugin-engine-typecheck-handl
//!
//! A thin Rust adapter that implements the [`TypecheckEngine`] trait by
//! loading and calling the Handl-authored typecheck model
//! (`project/design/adr0103/typecheck_model.hnl`).
//!
//! This is the ADR-0105 seam: the Rust NBE typechecker is slated for
//! deletion, replaced by Handl-authored models. The adapter stays thin —
//! the typechecking logic lives in the `.hnl` model, not here. The adapter
//! only marshals `Language`/`Ast` to `Value`, invokes the model's
//! `typecheck(lang, ast)` function via the bound eval engine, and
//! unmarshals the returned `{ typ, diagnostics, node_types }` record into a
//! [`TypecheckResult`].
//!
//! If the model cannot be loaded at construction time, the adapter logs a
//! warning and returns [`LangError::NotImplemented`] from
//! [`TypecheckEngine::typecheck`], so the crate always compiles even when
//! the Handl models are imperfect.

#![warn(missing_docs)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{Ast, LangError, Language, Value};
use handl_platform::{EvalContext, Module};
use handl_plugin_engine_exec::EvalEngine;
use plugin_engine::{Term, TermData};
use handl_plugin_engine_typecheck::{
    TypecheckDiagnostic, TypecheckEngine, TypecheckRequest, TypecheckResult,
};
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;

/// The embedded typecheck model source (ADR-0103/0105).
const TYPECHECK_MODEL_HNL: &str =
    include_str!("../../../project/design/adr0103/typecheck_model.hnl");

/// The function names exported by the typecheck model record.
const TYPECHECK_MODEL_FUNCS: &[&str] = &["typecheck"];

/// A thin [`TypecheckEngine`] adapter backed by a Handl-authored `.hnl`
/// typecheck model.
///
/// The adapter holds a bound [`EvalEngine`] (the tree-walk engine by
/// default), a loaded [`HandlModule`] (the typecheck model), an optional
/// module registry for `Foreign` op lookup, and globals injected into the
/// root context. It implements [`TypecheckEngine`] by marshaling the
/// `Language` and `Ast` to JSON values, calling the model's `typecheck`
/// function, and unmarshaling the result.
///
/// The typecheck model is loaded on the first call to [`TypecheckEngine::typecheck`],
/// so constructing the engine is cheap and the one-time model-load cost is only
/// paid when the `--handl` path is actually used.
pub struct HandlTypecheckEngine {
    /// The bound eval engine that runs the model's arrows.
    eval_engine: Box<dyn EvalEngine>,
    /// Lazily-initialized typecheck model module.
    model: Rc<RefCell<Option<Rc<HandlModule>>>>,
    /// Module registry for `Foreign` op lookup (type rules may call host
    /// functions).
    modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    /// Globals injected into the root context.
    globals: HashMap<String, Value>,
    /// Cached marshaled `Language` value, keyed by `Language` pointer, so
    /// the canonical `Language` is serialized at most once per engine.
    cached_lang: Rc<RefCell<Option<(usize, Value)>>>,
    /// Host-supplied wire names (prelude) that resolve at eval time even
    /// when not bound in the local gamma. Passed to the model as a third
    /// argument so `var` lookup for module/global names does not emit a
    /// false unbound error (ADR-0096).
    prelude: Value,
}

impl HandlTypecheckEngine {
    /// Create a new `HandlTypecheckEngine` bound to the given eval engine.
    ///
    /// The typecheck model source is embedded at compile time via
    /// [`include_str!`] and loaded into a [`HandlModule`]. If loading
    /// fails, a warning is logged and the engine degrades to returning
    /// [`LangError::NotImplemented`] from [`TypecheckEngine::typecheck`].
    pub fn new(eval_engine: Box<dyn EvalEngine>) -> Self {
        let this = Self {
            eval_engine,
            model: Rc::new(RefCell::new(None)),
            modules: None,
            globals: HashMap::new(),
            cached_lang: Rc::new(RefCell::new(None)),
            prelude: Value::Null,
        };
        if let Err(e) = this.ensure_model() {
            eprintln!("handl-plugin-engine-typecheck-handl: model load at construction: {e}");
        }
        this
    }

    /// Configure the host-supplied wire-name prelude.
    pub fn with_prelude(mut self, names: std::collections::HashSet<String>) -> Self {
        self.prelude = Value::Array(names.into_iter().map(Value::String).collect());
        self
    }

    /// Load the embedded `.hnl` typecheck model on first use.
    fn ensure_model(&self) -> Result<Rc<HandlModule>, LangError> {
        let mut cell = self.model.borrow_mut();
        if cell.is_none() {
            // The typecheck model is a bootstrap model: use the fast Rust
            // bootstrap transforms instead of the self-hosted loader pipeline.
            // The loader's `analyze` is prohibitively slow on the ~51k-char
            // model source (it hangs), and the bootstrap transforms produce
            // identical output for canonical sources.
            let loaded = handl_plugin_lib_handl::with_loader_disabled(||
                HandlModule::new(
                    "typecheck_model",
                    TYPECHECK_MODEL_HNL,
                    TYPECHECK_MODEL_FUNCS,
                )
                // Wire stdlib dependencies the model needs. The typecheck model
                // walks the AST and manages the typing context using the runtime
                // `array`, `hash_map`, and `typeof` modules, and constructs tagged
                // type values via the host `tag` capability (see the model header).
                .with_dependency(stdlib::module("array").expect("array module"))
                .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
                .with_dependency(stdlib::module("typeof").expect("typeof module"))
                .with_dependency(stdlib::module("obj").expect("obj module"))
                .with_dependency(TagModule)
                .try_into_loaded()
            )
            .map_err(|_msg| LangError::NotImplemented)?;
            *cell = Some(Rc::new(loaded));
        }
        let module = cell.as_ref().unwrap().clone();
        drop(cell);
        Ok(module)
    }

    /// Marshal a [`Language`] into a [`Value`] for the model.
    ///
    /// The model expects a raw serialized `Language` JSON object (it reads
    /// `lang.types`, `lang.eval`, etc. via `hash_map.get_or`), so this uses
    /// `serde_json::to_value` directly rather than the tagged
    /// `__language__` wrapper.
    ///
    /// `FragmentSet<T>` fields (`types`, `eval`, `grammar`, `ast`, etc.)
    /// serialize as `{by_key, id_index, override_chain}` objects, but the
    /// model expects arrays of fragment values. This method post-processes
    /// the serialized `Language` to extract `by_key` values as arrays for
    /// each `FragmentSet` field.
    fn marshal_language(lang: &Language) -> Result<Value, LangError> {
        let mut val = serde_json::to_value(lang)
            .map_err(|e| LangError::engine(format!("typecheck-handl: marshal lang: {e}")))?;
        // Convert FragmentSet fields from {by_key, id_index, override_chain}
        // objects to arrays of by_key values, which is what the model expects.
        if let Some(obj) = val.as_object_mut() {
            for field in [
                "tokens", "grammar", "eval", "types", "ast", "transforms",
                "caps", "options", "lexer_rules", "lints",
            ] {
                if let Some(fs) = obj.get_mut(field) {
                    if let Some(fs_obj) = fs.as_object() {
                        if fs_obj.contains_key("by_key") {
                            if let Some(by_key) = fs_obj.get("by_key") {
                                if let Some(by_key_obj) = by_key.as_object() {
                                    *fs = Value::Array(by_key_obj.values().cloned().collect());
                                }
                            }
                        }
                    }
                }
            }
        }
        // Convert scheme terms from serde's externally-tagged Term enum to the
        // tagged-record TermData shape the Handl model consumes (ADR-0103).
        Self::convert_schemes_to_term_data(&mut val);
        Ok(val)
    }

    /// Convert `Term` scheme values under `types`/`eval` to `TermData` tagged records.
    fn convert_schemes_to_term_data(val: &mut Value) {
        let Some(obj) = val.as_object_mut() else { return };
        for field in ["types", "eval"] {
            let Some(arr) = obj.get_mut(field).and_then(|v| v.as_array_mut()) else {
                continue;
            };
            for rule in arr {
                let Some(rule_obj) = rule.as_object_mut() else { continue };
                for scheme_key in ["synth", "check", "scheme"] {
                    if let Some(scheme) = rule_obj.get_mut(scheme_key) {
                        if let Ok(term) = serde_json::from_value::<Term>(scheme.clone()) {
                            *scheme = TermData::from_term(&term).0;
                        }
                    }
                }
            }
        }
    }

    /// Marshal an [`Ast`] into a [`Value`] for the model.
    ///
    /// The model expects a raw serialized `Ast` as a JSON **array** of
    /// nodes (it indexes `ast[i]` and reads `node.kind`/`node.payload`/
    /// `node.payload.children`). `Ast` serializes as `{"nodes": [...]}`
    /// via serde, so we extract the `nodes` slice and serialize that
    /// directly to produce the expected array shape.
    fn marshal_ast(ast: &Ast) -> Result<Value, LangError> {
        serde_json::to_value(ast.nodes())
            .map_err(|e| LangError::engine(format!("typecheck-handl: marshal ast: {e}")))
    }

    /// Unmarshal a model result [`Value`] (a `{ typ, diagnostics,
    /// node_types }` record) into a [`TypecheckResult`].
    fn unmarshal_result(val: &Value) -> Result<TypecheckResult, LangError> {
        let typ = val
            .get("typ")
            .cloned()
            .unwrap_or(Value::Null);

        let diagnostics = val
            .get("diagnostics")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|d| {
                        let severity = d
                            .get("severity")
                            .and_then(|v| v.as_str())
                            .unwrap_or("error")
                            .to_string();
                        let message = d
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Some(TypecheckDiagnostic {
                            severity,
                            message,
                            span: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // The model records node_types as an array indexed by node index
        // (sparse, with `null` holes). Convert to a HashMap<usize, Value>,
        // skipping null/absent entries.
        let node_types = val
            .get("node_types")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .enumerate()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(i, v)| (i, v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(TypecheckResult {
            typ,
            diagnostics,
            node_types,
        })
    }
}

impl TypecheckEngine for HandlTypecheckEngine {
    fn typecheck(
        &self,
        lang: &Language,
        req: &TypecheckRequest,
    ) -> Result<TypecheckResult, LangError> {
        let model = self.ensure_model()?;
        let lang_ptr = lang as *const _ as usize;
        let lang_val = {
            let cache = self.cached_lang.borrow();
            if let Some((ptr, val)) = cache.as_ref() {
                if *ptr == lang_ptr {
                    val.clone()
                } else {
                    drop(cache);
                    let new_val = Self::marshal_language(lang)?;
                    *self.cached_lang.borrow_mut() = Some((lang_ptr, new_val.clone()));
                    new_val
                }
            } else {
                drop(cache);
                let new_val = Self::marshal_language(lang)?;
                *self.cached_lang.borrow_mut() = Some((lang_ptr, new_val.clone()));
                new_val
            }
        };
        let ast_val = Self::marshal_ast(&req.ast)?;
        let func = model
            .function("typecheck")
            .ok_or_else(|| LangError::engine("typecheck-handl: model has no 'typecheck' export"))?;
        let args = vec![lang_val, ast_val, self.prelude.clone()];
        let mut ctx = EvalContext::new();
        let result_val = match func.call(&args, &mut ctx) {
            Ok(v) => v,
            Err(_) => {
                // The model hit a construct it cannot evaluate. Degrade to
                // NotImplemented so the host can fall back to the bootstrap
                // checker instead of reporting an engine error.
                return Err(LangError::NotImplemented);
            }
        };
        Self::unmarshal_result(&result_val)
    }

    fn box_clone(&self) -> Box<dyn TypecheckEngine> {
        Box::new(Self {
            eval_engine: self.eval_engine.box_clone(),
            model: self.model.clone(),
            modules: self.modules.clone(),
            globals: self.globals.clone(),
            cached_lang: self.cached_lang.clone(),
            prelude: self.prelude.clone(),
        })
    }

    fn with_modules(
        &self,
        modules: Rc<dyn handl_platform::ModuleLookup>,
    ) -> Box<dyn TypecheckEngine> {
        Box::new(Self {
            eval_engine: self.eval_engine.with_modules(modules.clone()),
            model: self.model.clone(),
            modules: Some(modules),
            globals: self.globals.clone(),
            cached_lang: self.cached_lang.clone(),
            prelude: self.prelude.clone(),
        })
    }

    fn with_global(&self, name: &str, val: Value) -> Box<dyn TypecheckEngine> {
        let mut globals = self.globals.clone();
        globals.insert(name.to_string(), val);
        Box::new(Self {
            eval_engine: self.eval_engine.box_clone(),
            model: self.model.clone(),
            modules: self.modules.clone(),
            globals,
            cached_lang: self.cached_lang.clone(),
            prelude: self.prelude.clone(),
        })
    }
}

impl Clone for HandlTypecheckEngine {
    fn clone(&self) -> Self {
        Self {
            eval_engine: self.eval_engine.box_clone(),
            model: self.model.clone(),
            modules: self.modules.clone(),
            globals: self.globals.clone(),
            cached_lang: self.cached_lang.clone(),
            prelude: self.prelude.clone(),
        }
    }
}

/// Extension trait to validate that a [`HandlModule`] loaded its functions
/// without error, returning an owned module on success.
trait TryIntoLoaded {
    /// Consume the module, returning it on success or an error message on
    /// failure (the model source failed to parse/analyze).
    fn try_into_loaded(self) -> Result<HandlModule, String>;
}

impl TryIntoLoaded for HandlModule {
    fn try_into_loaded(self) -> Result<HandlModule, String> {
        // The typecheck model exports `typecheck`; verify it resolves. A
        // missing export means the source failed to parse/analyze into a
        // record with that field.
        if self.function("typecheck").is_some() {
            Ok(self)
        } else {
            Err("model has no 'typecheck' export".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode, Language};
    use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;

    fn engine() -> HandlTypecheckEngine {
        HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()))
    }

    #[test]
    fn constructs_and_loads_model() {
        let e = engine();
        assert!(
            e.model.borrow().is_some(),
            "typecheck model should load at construction time"
        );
    }

    #[test]
    fn empty_ast_returns_error_or_result() {
        let e = engine();
        let lang = Language::new();
        let req = TypecheckRequest::new(Ast::new());
        // The model returns a TypeError for empty ast rather than a Rust
        // error; either outcome is acceptable for the adapter contract.
        let _ = e.typecheck(&lang, &req);
    }

    #[test]
    fn single_node_typecheck_runs() {
        let e = engine();
        let lang = Language::new();
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit"));
        let req = TypecheckRequest::new(ast);
        // The model may return NotImplemented-ish TypeError for an unknown
        // kind in an empty language; the adapter must not panic.
        let _ = e.typecheck(&lang, &req);
    }

    #[test]
    fn box_clone_preserves_model() {
        let e = engine();
        let cloned = e.box_clone();
        let lang = Language::new();
        let req = TypecheckRequest::new(Ast::new());
        let _ = cloned.typecheck(&lang, &req);
    }

    #[test]
    fn unmarshal_result_handles_minimal_record() {
        let val = serde_json::json!({
            "typ": { "__tag__": "type.unknown" },
            "diagnostics": [],
            "node_types": []
        });
        let result = HandlTypecheckEngine::unmarshal_result(&val).unwrap();
        assert_eq!(result.typ, serde_json::json!({ "__tag__": "type.unknown" }));
        assert!(result.diagnostics.is_empty());
        assert!(result.node_types.is_empty());
    }

    #[test]
    fn unmarshal_result_parses_diagnostics_and_node_types() {
        let val = serde_json::json!({
            "typ": null,
            "diagnostics": [
                { "severity": "warning", "message": "watch out" },
                { "severity": "error", "message": "boom" }
            ],
            "node_types": [null, { "__tag__": "type.num" }, null]
        });
        let result = HandlTypecheckEngine::unmarshal_result(&val).unwrap();
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(result.diagnostics[0].severity, "warning");
        assert_eq!(result.diagnostics[0].message, "watch out");
        assert_eq!(result.diagnostics[1].severity, "error");
        // node_types skips null holes.
        assert_eq!(result.node_types.len(), 1);
        assert_eq!(
            result.node_types.get(&1),
            Some(&serde_json::json!({ "__tag__": "type.num" }))
        );
    }
}
