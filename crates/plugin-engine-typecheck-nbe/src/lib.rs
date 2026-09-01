//! # handl-plugin-engine-typecheck-nbe
//!
//! A **data-driven, normalization-based typecheck engine** implementing the
//! [`TypecheckEngine`] trait (ADR-0054 — Generic Typecheck Engine).
//!
//! This crate is a **new, minimal, real** engine that proves the ADR-0054
//! architecture end-to-end. It is **not** a rewrite of the legacy
//! `plugin-typecheck-nbe` crate; the full rewrite is gated on spikes per the
//! ADR's Status section.
//!
//! ## Architecture (ADR-0054 §4)
//!
//! The engine:
//!
//! 1. Implements [`TypecheckEngine`].
//! 2. Holds a bound [`EvalEngine`] (for value-level computation in type rules
//!    — "types are values", ADR-0032). The bound eval engine is the pure-eval
//!    path: effectful `Foreign` ops are rejected. For this initial impl, the
//!    host constructs the typecheck engine with a pure eval engine; the
//!    purity boundary is the host's responsibility.
//! 3. Reads `lang.types[node.kind]` → [`TypeRuleDef`] → [`Scheme`] (the
//!    `synth` scheme; `check` is optional) and interprets it.
//! 4. Implements the **judgment ops** (ADR-0054 §2) as `Arr` ops with
//!    [`ArrSource::Judgment`].
//! 5. Produces **tagged type values** via the [`TypeRegistry`] from
//!    `plugin-lib-tag` (ADR-0055): `construct` mints
//!    `{"__tag__":"type.<tag>","__registry__":"handl.canonical",...}`,
//!    `match_type` dispatches on the tag.
//!
//! ## Threading convention
//!
//! [`Scheme::Seq`] threads the `fst` result into `snd` as the `incoming`
//! value. The last `Seq`'s result is the scheme's result. To recall a
//! previously-computed value, use the `bind` / `result` judgment ops:
//!
//! ```text
//! synth_child(0) >>> bind("t") >>> ... >>> result("t")
//! ```
//!
//! `bind(name)` stores `incoming` under `name`; `result(name)` recalls it.
//!
//! ## Choice convention
//!
//! [`Scheme::Choice`] routes on an Either value encoded as
//! `{"__choice__": "left"|"right", "value": v}`. `l` handles `left`, `r`
//! handles `right`.
//!
//! ## Limitations of this initial impl
//!
//! - [`Scheme::Loop`] and [`Scheme::App`] are not supported (return
//!   [`LangError::NotImplemented`]).
//! - `Native`/`Foreign` `Arr` ops in type rules are not supported (except
//!   `const` and `id`, which are trivial and common). They return an engine
//!   error. The ADR notes the op set is "refined during implementation".
//! - `unify` uses structural equality plus metavariable solving; no
//!   occurs-check or higher-rank polymorphism.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{ArrSource, Ast, AstNode, AstRef, LangError, Language, Scheme, Value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest, TypecheckResult};
use handl_plugin_lib_tag::TypeRegistry;

/// The key under which a metavariable id is encoded in a type value.
///
/// A metavariable is encoded as `{"__meta__": <id>}`.
const META_KEY: &str = "__meta__";

/// The key under which a `Choice` Either's side is encoded.
///
/// An Either is encoded as `{"__choice__": "left"|"right", "value": v}`.
const CHOICE_KEY: &str = "__choice__";

/// The key under which a `Choice` Either's payload value is encoded.
const CHOICE_VALUE_KEY: &str = "value";

/// A data-driven, normalization-based typecheck engine (ADR-0054).
///
/// Implements [`TypecheckEngine`] by interpreting the arrow-calculus
/// [`Scheme`]s found in `lang.types[node.kind]`. Value-level computation
/// within type rules is delegated to a bound [`EvalEngine`] (the pure-eval
/// path); judgment ops are interpreted directly.
pub struct NbeTypecheckEngine {
    /// The bound eval engine for value-level computation in type rules.
    eval_engine: Box<dyn EvalEngine>,
    /// The type registry id stamped into minted type values
    /// (default `"handl.canonical"`).
    registry_id: String,
}

impl NbeTypecheckEngine {
    /// Create a new typecheck engine bound to the given eval engine.
    ///
    /// The eval engine should be the pure-eval path (effectful `Foreign`
    /// ops are rejected); the purity boundary is the host's responsibility.
    /// The registry id defaults to `"handl.canonical"`.
    pub fn new(eval_engine: Box<dyn EvalEngine>) -> Self {
        Self {
            eval_engine,
            registry_id: "handl.canonical".to_string(),
        }
    }

    /// Builder: set the type registry id (stamped into minted type values).
    pub fn with_registry_id(mut self, id: impl Into<String>) -> Self {
        self.registry_id = id.into();
        self
    }

    /// Synthesize the type of a node by looking up its type rule and
    /// interpreting the `synth` scheme.
    fn synth_node(
        &self,
        lang: &Language,
        ast: &Ast,
        node_ref: AstRef,
        ctx: &mut TypecheckContext,
    ) -> Result<Value, LangError> {
        let node = ast.get(node_ref).ok_or_else(|| {
            LangError::Engine(format!(
                "typecheck: node index {} out of bounds",
                node_ref.index()
            ))
        })?;
        let rule = lang.types.get_by_identity(&node.kind).ok_or_else(|| {
            LangError::Engine(format!(
                "typecheck: no type rule for kind '{}'",
                node.kind
            ))
        })?;
        self.eval_type_scheme(lang, ast, node_ref, &rule.synth, ctx, None)
    }

    /// Check a node against an expected type by interpreting the `check`
    /// scheme (or synth-then-unify if no `check` scheme exists).
    fn check_node(
        &self,
        lang: &Language,
        ast: &Ast,
        node_ref: AstRef,
        expected: &Value,
        ctx: &mut TypecheckContext,
    ) -> Result<(), LangError> {
        let node = ast.get(node_ref).ok_or_else(|| {
            LangError::Engine(format!(
                "typecheck: node index {} out of bounds",
                node_ref.index()
            ))
        })?;
        let rule = lang.types.get_by_identity(&node.kind).ok_or_else(|| {
            LangError::Engine(format!(
                "typecheck: no type rule for kind '{}'",
                node.kind
            ))
        })?;
        if let Some(check_scheme) = &rule.check {
            self.eval_type_scheme(
                lang,
                ast,
                node_ref,
                check_scheme,
                ctx,
                Some(expected.clone()),
            )?;
            Ok(())
        } else {
            let ty = self.synth_node(lang, ast, node_ref, ctx)?;
            self.unify(&ty, expected, ctx)?;
            Ok(())
        }
    }

    /// Interpret a type-rule [`Scheme`] combinator tree.
    ///
    /// Mirrors the tree-walk eval engine's combinator structure, but
    /// handles `Arr` with [`ArrSource::Judgment`] here (judgment ops) and
    /// delegates `Native`/`Foreign` to the bound eval engine (only `const`
    /// and `id` are supported in this initial impl).
    fn eval_type_scheme(
        &self,
        lang: &Language,
        ast: &Ast,
        node_ref: AstRef,
        scheme: &Scheme,
        ctx: &mut TypecheckContext,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        match scheme {
            Scheme::Arr {
                op,
                args,
                source,
                lit,
            } => match source {
                ArrSource::Judgment => {
                    self.eval_judgment(op, args, lit.as_ref(), lang, ast, node_ref, ctx, incoming)
                }
                ArrSource::Native | ArrSource::Foreign(_) => {
                    self.eval_native_arr(op, lit.as_ref(), incoming)
                }
            },
            Scheme::Seq { fst, snd } => {
                let fst_val =
                    self.eval_type_scheme(lang, ast, node_ref, fst, ctx, incoming)?;
                self.eval_type_scheme(lang, ast, node_ref, snd, ctx, Some(fst_val))
            }
            Scheme::First { inner } => {
                let inc = incoming.ok_or_else(|| {
                    LangError::Engine("typecheck: First: missing incoming value".into())
                })?;
                let pair = inc.as_array().ok_or_else(|| {
                    LangError::Engine("typecheck: First: incoming is not an array".into())
                })?;
                if pair.len() != 2 {
                    return Err(LangError::Engine(
                        "typecheck: First: incoming pair must have 2 elements".into(),
                    ));
                }
                let a = pair[0].clone();
                let b = pair[1].clone();
                let result =
                    self.eval_type_scheme(lang, ast, node_ref, inner, ctx, Some(a))?;
                Ok(Value::Array(vec![result, b]))
            }
            Scheme::Choice { l, r } => {
                let inc = incoming.ok_or_else(|| {
                    LangError::Engine("typecheck: Choice: missing incoming value".into())
                })?;
                let obj = inc.as_object().ok_or_else(|| {
                    LangError::Engine("typecheck: Choice: incoming is not an object".into())
                })?;
                let side = obj
                    .get(CHOICE_KEY)
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: Choice: incoming missing __choice__ key".into(),
                        )
                    })?;
                let value = obj
                    .get(CHOICE_VALUE_KEY)
                    .cloned()
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: Choice: incoming missing value key".into(),
                        )
                    })?;
                match side {
                    "left" => self.eval_type_scheme(lang, ast, node_ref, l, ctx, Some(value)),
                    "right" => self.eval_type_scheme(lang, ast, node_ref, r, ctx, Some(value)),
                    other => Err(LangError::Engine(format!(
                        "typecheck: Choice: unknown side '{}'",
                        other
                    ))),
                }
            }
            Scheme::Loop { .. } => Err(LangError::NotImplemented),
            Scheme::App { .. } => Err(LangError::NotImplemented),
        }
    }

    /// Evaluate a `Native`/`Foreign` `Arr` op within a type rule.
    ///
    /// Only `const` and `id` are supported in this initial impl; all other
    /// native/foreign ops return an engine error (the ADR notes the op set
    /// is refined during implementation).
    fn eval_native_arr(
        &self,
        op: &str,
        lit: Option<&Value>,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        match op {
            "const" => Ok(lit.cloned().unwrap_or(Value::Null)),
            "id" => Ok(incoming.unwrap_or(Value::Null)),
            other => Err(LangError::Engine(format!(
                "typecheck: native/foreign op '{}' not supported in type rules yet",
                other
            ))),
        }
    }

    /// Dispatch a judgment op (ADR-0054 §2).
    ///
    /// Evaluates `args` sub-schemes recursively, then performs the judgment
    /// operation. The `lit` payload carries inline operands (e.g. the child
    /// index for `synth_child`, the name for `bind`/`result`/`lookup`).
    #[allow(clippy::too_many_arguments)]
    fn eval_judgment(
        &self,
        op: &str,
        args: &[Scheme],
        lit: Option<&Value>,
        lang: &Language,
        ast: &Ast,
        node_ref: AstRef,
        ctx: &mut TypecheckContext,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        match op {
            "synth_child" | "synth_type_child" => {
                let idx = lit
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine(format!(
                            "typecheck: {} requires a u64 lit (child index)",
                            op
                        ))
                    })?;
                let child = child_ref(ast, node_ref, idx)?;
                self.synth_node(lang, ast, child, ctx)
            }
            "check_child" => {
                let idx = lit
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: check_child requires a u64 lit (child index)".into(),
                        )
                    })?;
                let expected = if args.is_empty() {
                    incoming.clone().unwrap_or(Value::Null)
                } else {
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?
                };
                let child = child_ref(ast, node_ref, idx)?;
                self.check_node(lang, ast, child, &expected, ctx)?;
                Ok(Value::Null)
            }
            "eval_child" | "proj_child" => {
                let idx = lit
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine(format!(
                            "typecheck: {} requires a u64 lit (child index)",
                            op
                        ))
                    })?;
                let child = child_ref(ast, node_ref, idx)?;
                let sub = ast.subtree(child)?;
                let req = EvalRequest::new(sub);
                self.eval_engine.eval(lang, &req)
            }
            "unify" => {
                if args.len() == 2 {
                    let a =
                        self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                    let b =
                        self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                    self.unify(&a, &b, ctx)?;
                } else {
                    let inc = incoming.unwrap_or(Value::Null);
                    let expected = lit.cloned().unwrap_or(Value::Null);
                    self.unify(&inc, &expected, ctx)?;
                }
                Ok(Value::Null)
            }
            "extend" => {
                let name = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: extend requires a string lit (name)".into(),
                        )
                    })?
                    .to_string();
                let ty = if args.is_empty() {
                    incoming.clone().unwrap_or(Value::Null)
                } else {
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?
                };
                ctx.extend(name, ty);
                Ok(incoming.unwrap_or(Value::Null))
            }
            "lookup" => {
                // Read the name from `lit` when a non-null literal is
                // present; otherwise read from `incoming` (a string
                // `Value`). The `incoming` path enables `var` rules that
                // thread the node's payload (the variable name) via
                // `node_payload >>> lookup` (ADR-0054 task 4).
                let name = match lit.and_then(|v| if v.is_null() { None } else { Some(v) }) {
                    Some(v) => v.as_str().ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: lookup: lit must be a string (name)".into(),
                        )
                    })?,
                    None => incoming.as_ref().and_then(|v| v.as_str()).ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: lookup: requires a string lit or string incoming (name)"
                                .into(),
                        )
                    })?,
                };
                ctx.lookup(name)
                    .cloned()
                    .ok_or_else(|| LangError::Engine(format!(
                        "typecheck: lookup: unbound variable '{}'",
                        name
                    )))
            }
            "lookup_var" => {
                let idx = lit
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: lookup_var requires a u64 lit (index)".into(),
                        )
                    })?;
                ctx.lookup_index(idx as usize)
                    .cloned()
                    .ok_or_else(|| LangError::Engine(format!(
                        "typecheck: lookup_var: no binding at index {}",
                        idx
                    )))
            }
            "fresh_meta" => {
                let id = ctx.fresh_meta();
                Ok(meta_value(id))
            }
            "solve_meta" => {
                let id = if args.is_empty() {
                    lit.and_then(|v| v.as_u64())
                        .ok_or_else(|| {
                            LangError::Engine(
                                "typecheck: solve_meta requires a u64 id (lit or arg)".into(),
                            )
                        })?
                } else {
                    let id_val = self.eval_type_scheme(
                        lang, ast, node_ref, &args[0], ctx, incoming.clone(),
                    )?;
                    id_val.as_u64().ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: solve_meta: id arg is not a u64".into(),
                        )
                    })?
                };
                let val = if args.len() >= 2 {
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?
                } else {
                    incoming.unwrap_or(Value::Null)
                };
                ctx.solve_meta(id, val);
                Ok(Value::Null)
            }
            "quote" | "normalize" => {
                if args.is_empty() {
                    Ok(incoming.unwrap_or(Value::Null))
                } else {
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming)
                }
            }
            "construct" => {
                let tag = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: construct requires a string lit (tag name)".into(),
                        )
                    })?;
                let mut payload = serde_json::Map::new();
                for arg in args {
                    let val =
                        self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?;
                    if let Value::Object(map) = val {
                        for (k, v) in map {
                            payload.insert(k, v);
                        }
                    } else if val != Value::Null {
                        return Err(LangError::Engine(format!(
                            "typecheck: construct: arg produced a non-object value: {:?}",
                            val
                        )));
                    }
                }
                let payload_val = if payload.is_empty() {
                    Value::Null
                } else {
                    Value::Object(payload)
                };
                ctx.registry
                    .construct(tag, &payload_val)
                    .ok_or_else(|| {
                        LangError::Engine(format!(
                            "typecheck: construct: failed to mint type value for tag '{}'",
                            tag
                        ))
                    })
            }
            "match_type" => {
                if args.len() < 2 {
                    return Err(LangError::Engine(
                        "typecheck: match_type requires 2 args (value, cases)".into(),
                    ));
                }
                let value =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let cases =
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                Ok(ctx.registry.match_type(&value, &cases).unwrap_or(Value::Null))
            }
            "bind" => {
                let name = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: bind requires a string lit (name)".into(),
                        )
                    })?
                    .to_string();
                let val = incoming.unwrap_or(Value::Null);
                ctx.bind_result(name, val.clone());
                Ok(val)
            }
            "result" => {
                let name = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "typecheck: result requires a string lit (name)".into(),
                        )
                    })?;
                ctx.lookup_result(name)
                    .cloned()
                    .ok_or_else(|| {
                        LangError::Engine(format!(
                            "typecheck: result: no stored result for '{}'",
                            name
                        ))
                    })
            }
            "node_payload" => {
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::Engine(format!(
                        "typecheck: node_payload: node index {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                Ok(node.payload.clone())
            }
            other => Err(LangError::Engine(format!(
                "typecheck: unknown judgment op '{}'",
                other
            ))),
        }
    }

    /// Unify two type values (structural equality + metavariable solving).
    ///
    /// If both sides are equal, succeeds. If one side is an unsolved
    /// metavariable (`{"__meta__": id}`), solves it to the other side.
    /// Otherwise returns an engine error.
    fn unify(
        &self,
        a: &Value,
        b: &Value,
        ctx: &mut TypecheckContext,
    ) -> Result<(), LangError> {
        if a == b {
            return Ok(());
        }
        if let Some(id) = extract_meta_id(a) {
            ctx.solve_meta(id, b.clone());
            return Ok(());
        }
        if let Some(id) = extract_meta_id(b) {
            ctx.solve_meta(id, a.clone());
            return Ok(());
        }
        Err(LangError::Engine(format!(
            "typecheck: cannot unify {:?} with {:?}",
            a, b
        )))
    }
}

impl TypecheckEngine for NbeTypecheckEngine {
    fn typecheck(
        &self,
        lang: &Language,
        req: &TypecheckRequest,
    ) -> Result<TypecheckResult, LangError> {
        if req.ast.is_empty() {
            return Err(LangError::Engine("typecheck: empty ast".into()));
        }
        // The root node is the LAST node in the arena (Pratt convention).
        let root_ref = AstRef::new(req.ast.len() - 1);
        let mut ctx = TypecheckContext::new(&self.registry_id);
        let typ = self.synth_node(lang, &req.ast, root_ref, &mut ctx)?;
        Ok(TypecheckResult {
            typ,
            diagnostics: vec![],
        })
    }

    fn box_clone(&self) -> Box<dyn TypecheckEngine> {
        Box::new(Self {
            eval_engine: self.eval_engine.box_clone(),
            registry_id: self.registry_id.clone(),
        })
    }

    fn with_modules(
        &self,
        modules: Rc<dyn handl_platform::ModuleLookup>,
    ) -> Box<dyn TypecheckEngine> {
        Box::new(Self {
            eval_engine: self.eval_engine.with_modules(modules),
            registry_id: self.registry_id.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// TypecheckContext
// ---------------------------------------------------------------------------

/// Internal judgment state: metavariable store, typing context Γ, named
/// results, and the host type registry.
struct TypecheckContext {
    /// Metavariable store: id → solved value (`None` = unsolved).
    metas: Vec<Option<Value>>,
    /// Typing context Γ: list of `(name, type_value)` pairs.
    gamma: Vec<(String, Value)>,
    /// Named results store (for `bind` / `result`).
    results: HashMap<String, Value>,
    /// The host type registry (for `construct` / `match_type`).
    registry: TypeRegistry,
}

impl TypecheckContext {
    /// Create a fresh context with the given registry id.
    fn new(registry_id: &str) -> Self {
        Self {
            metas: Vec::new(),
            gamma: Vec::new(),
            results: HashMap::new(),
            registry: TypeRegistry::new(registry_id),
        }
    }

    /// Allocate a fresh metavariable, returning its id.
    fn fresh_meta(&mut self) -> u64 {
        let id = self.metas.len() as u64;
        self.metas.push(None);
        id
    }

    /// Solve a metavariable to a value.
    fn solve_meta(&mut self, id: u64, val: Value) {
        if (id as usize) < self.metas.len() {
            self.metas[id as usize] = Some(val);
        }
    }

    /// Look up a solved metavariable's value.
    ///
    /// Part of the metavariable API (ADR-0054 §2). Not yet exercised by a
    /// judgment op in this initial impl; reserved for future `read_meta` /
    /// `quote` refinements.
    #[allow(dead_code)]
    fn lookup_meta(&self, id: u64) -> Option<&Value> {
        self.metas.get(id as usize).and_then(|opt| opt.as_ref())
    }

    /// Extend Γ with a `(name, type)` binding.
    fn extend(&mut self, name: String, ty: Value) {
        self.gamma.push((name, ty));
    }

    /// Look up a variable's type in Γ (reverse order — innermost first).
    fn lookup(&self, name: &str) -> Option<&Value> {
        self.gamma
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    /// Look up a binding by index in Γ (0 = outermost).
    fn lookup_index(&self, idx: usize) -> Option<&Value> {
        self.gamma.get(idx).map(|(_, v)| v)
    }

    /// Store a named result (for `bind`).
    fn bind_result(&mut self, name: String, val: Value) {
        self.results.insert(name, val);
    }

    /// Recall a named result (for `result`).
    fn lookup_result(&self, name: &str) -> Option<&Value> {
        self.results.get(name)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get the [`AstRef`] of child `idx` of the node at `node_ref`.
///
/// Reads `node.payload["children"][idx]` as a u64 arena index.
fn child_ref(ast: &Ast, node_ref: AstRef, idx: u64) -> Result<AstRef, LangError> {
    let node: &AstNode = ast.get(node_ref).ok_or_else(|| {
        LangError::Engine(format!(
            "typecheck: child_ref: node index {} out of bounds",
            node_ref.index()
        ))
    })?;
    let children = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .ok_or_else(|| {
            LangError::Engine(format!(
                "typecheck: child_ref: node kind '{}' has no children array",
                node.kind
            ))
        })?;
    let child_idx = children
        .get(idx as usize)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            LangError::Engine(format!(
                "typecheck: child_ref: child index {} out of bounds or not a u64",
                idx
            ))
        })?;
    Ok(AstRef::new(child_idx as usize))
}

/// Encode a metavariable id as `{"__meta__": id}`.
fn meta_value(id: u64) -> Value {
    let mut map = serde_json::Map::new();
    map.insert(META_KEY.to_string(), Value::from(id));
    Value::Object(map)
}

/// Extract a metavariable id from a value, if it is `{"__meta__": id}`
/// with a single key and a u64 value.
fn extract_meta_id(v: &Value) -> Option<u64> {
    let obj = v.as_object()?;
    if obj.len() != 1 {
        return None;
    }
    obj.get(META_KEY).and_then(|id| id.as_u64())
}

#[cfg(test)]
mod tests;
