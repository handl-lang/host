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
//! - `Native` `Arr` ops in type rules are supported only for `const` and
//!   `id` (trivial, common); other native ops return an engine error. The
//!   ADR notes the op set is "refined during implementation". `Foreign`
//!   `Arr` ops in type rules are **rejected** (ADR-0054 pure-eval path):
//!   type rules are static data and must never invoke effectful host
//!   calls. The `eval_child`/`proj_child` judgment ops additionally
//!   perform a static purity check on the child's AST subtree (gated by
//!   the `pure` flag, default `true`).
//! - `unify` uses structural equality, metavariable solving, and recursive
//!   structural unification of tagged type values with the same tag from the
//!   same registry (ADR-0054 legacy migration); no occurs-check or
//!   higher-rank polymorphism. For dependent Pi/Sigma types with
//!   `body_closure` fields, `unify` skips the closure marker (only unifies
//!   `param`); use the `unify_pi` judgment op for full closure-based
//!   unification (fresh var, apply both, unify bodies).
//! - Dependent-type closure machinery (`construct_pi`, `construct_sigma`,
//!   `pi_apply`, `unify_pi`, `check_pi_body`) brings NBE closures into the
//!   data-driven engine for Pi/Sigma types (ADR-0054). A Pi/Sigma type value
//!   carries a `body_closure` field recording the variable name and the AST
//!   child index of the body; closure application marks Γ, extends it, synths
//!   the body child, and restores Γ — mirroring the legacy `Closure::apply`.
//! - Forall (parametric polymorphism) and Vau (fexpr) type support
//!   (`construct_forall`, `forall_apply`, `unify_forall`,
//!   `construct_vau_type`, `vau_apply`, `unify_vau`) extends the closure
//!   machinery to `type.forall` (universal quantification — parameter is a
//!   type, field is `kind`) and `type.vau` (unevaluated-parameter functions —
//!   like `type.pi` but marked as fexpr). Both reuse the same
//!   `body_closure`/`apply_type_closure` machinery as Pi/Sigma (ADR-0054).
//! - Equality type support (`construct_eq`, `construct_empty`, `check_refl`,
//!   `eq_reduce`) brings propositional equality into the data-driven engine
//!   (ADR-0054): `Eq(a, b)` is the equality type (inhabited by `Refl` iff
//!   `a == b`), `Refl` is the proof, and `Empty` is the empty type
//!   (contradiction, used when equality fails). These mirror the legacy
//!   `Value::Eq`, `Value::Refl`, `Value::Empty`.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{
    infer_effect_row, ArrSource, Ast, AstNode, AstRef, EffectRow, LangError, Language, Scheme,
    Value, Verdict,
};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_typecheck::{
    TypecheckDiagnostic, TypecheckEngine, TypecheckRequest, TypecheckResult,
};
use handl_plugin_lib_tag::TypeRegistry;

mod predicate_eval;
mod totality_check;

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

/// FFI marshalling type tags (ADR-0083).
const FFI_MARSHALLING_TYPES: &[&str] = &[
    "type.cstr",   // const char* -> String
    "type.i32",    // int32_t -> Number
    "type.i64",    // int64_t -> Number
    "type.f64",    // double -> Number
    "type.buf",    // void* + size_t -> Array<Byte>
    "type.ptr",    // void* -> opaque Handle
    "type.void",   // void -> Unit
    "type.handle", // void* -> opaque Handle (resource table)
];

/// Check if a type value's tag is an FFI-marshalable type (ADR-0083).
fn is_marshalable(registry: &TypeRegistry, ty: &Value) -> bool {
    registry
        .as_type_tag(ty)
        .map(|tag| FFI_MARSHALLING_TYPES.contains(&tag))
        .unwrap_or(false)
}

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
    /// Whether the pure-eval path is enforced (ADR-0054). When `true`
    /// (the default), the engine rejects effectful `Foreign` ops in
    /// `eval_child`/`proj_child` judgment ops via a static purity check
    /// on the child's AST subtree. When `false`, the host explicitly opts
    /// into an impure typecheck and the purity check is skipped. The
    /// rejection of `Foreign` ops directly authored in a type rule's
    /// `Scheme` is **always** enforced regardless of this flag — type
    /// rules themselves must never contain `Foreign` ops.
    pure: bool,
    /// Module registry for Foreign op type signature lookup (ADR-0083).
    modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
}

impl NbeTypecheckEngine {
    /// Create a new typecheck engine bound to the given eval engine.
    ///
    /// The eval engine should be the pure-eval path (effectful `Foreign`
    /// ops are rejected); the purity boundary is the host's responsibility.
    /// The registry id defaults to `"handl.canonical"`. The engine defaults
    /// to `pure: true` (ADR-0054 pure-eval path).
    pub fn new(eval_engine: Box<dyn EvalEngine>) -> Self {
        Self {
            eval_engine,
            registry_id: "handl.canonical".to_string(),
            pure: true,
            modules: None,
        }
    }

    /// Builder: set the type registry id (stamped into minted type values).
    pub fn with_registry_id(mut self, id: impl Into<String>) -> Self {
        self.registry_id = id.into();
        self
    }

    /// Builder: set the `pure` flag (ADR-0054). When `false`, the host
    /// explicitly opts into an impure typecheck and the `eval_child`/
    /// `proj_child` purity check is skipped. The rejection of `Foreign`
    /// ops in type rule schemes is always enforced.
    pub fn with_pure(self, pure: bool) -> Self {
        Self {
            pure,
            modules: self.modules.clone(),
            ..self
        }
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
            LangError::engine(format!(
                "typecheck: node index {} out of bounds",
                node_ref.index()
            ))
        })?;
        let rule = lang.types.get_by_identity(&node.kind).ok_or_else(|| {
            LangError::engine_node(
                format!("typecheck: no type rule for kind '{}'", node.kind),
                node,
            )
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
            LangError::engine(format!(
                "typecheck: node index {} out of bounds",
                node_ref.index()
            ))
        })?;
        let rule = lang.types.get_by_identity(&node.kind).ok_or_else(|| {
            LangError::engine_node(
                format!("typecheck: no type rule for kind '{}'", node.kind),
                node,
            )
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
                ArrSource::Native | ArrSource::DynamicForeign => {
                    self.eval_native_arr(op, lit.as_ref(), incoming)
                }
                ArrSource::Foreign(name) => {
                    // ADR-0054 pure-eval path: type rules themselves must
                    // never contain `Foreign` ops — they are effectful host
                    // calls and have no place in a pure typecheck scheme.
                    // This is enforced unconditionally (regardless of the
                    // `pure` flag), since type rules are static data, not
                    // user programs.
                    Err(LangError::engine(format!(
                        "typecheck: foreign op '{}' is not allowed in type rules (pure-eval path, ADR-0054)",
                        name
                    )))
                }
            },
            Scheme::Seq { fst, snd } => {
                let fst_val = self.eval_type_scheme(lang, ast, node_ref, fst, ctx, incoming)?;
                self.eval_type_scheme(lang, ast, node_ref, snd, ctx, Some(fst_val))
            }
            Scheme::First { inner } => {
                let inc = incoming
                    .ok_or_else(|| LangError::engine("typecheck: First: missing incoming value"))?;
                let pair = inc.as_array().ok_or_else(|| {
                    LangError::engine("typecheck: First: incoming is not an array")
                })?;
                if pair.len() != 2 {
                    return Err(LangError::engine(
                        "typecheck: First: incoming pair must have 2 elements",
                    ));
                }
                let a = pair[0].clone();
                let b = pair[1].clone();
                let result = self.eval_type_scheme(lang, ast, node_ref, inner, ctx, Some(a))?;
                Ok(Value::Array(vec![result, b]))
            }
            Scheme::Choice { l, r } => {
                let inc = incoming.ok_or_else(|| {
                    LangError::engine("typecheck: Choice: missing incoming value")
                })?;
                let obj = inc.as_object().ok_or_else(|| {
                    LangError::engine("typecheck: Choice: incoming is not an object")
                })?;
                let side = obj
                    .get(CHOICE_KEY)
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| {
                        LangError::engine("typecheck: Choice: incoming missing __choice__ key")
                    })?;
                let value = obj.get(CHOICE_VALUE_KEY).cloned().ok_or_else(|| {
                    LangError::engine("typecheck: Choice: incoming missing value key")
                })?;
                match side {
                    "left" => self.eval_type_scheme(lang, ast, node_ref, l, ctx, Some(value)),
                    "right" => self.eval_type_scheme(lang, ast, node_ref, r, ctx, Some(value)),
                    other => Err(LangError::engine(format!(
                        "typecheck: Choice: unknown side '{}'",
                        other
                    ))),
                }
            }
            Scheme::Loop { .. } => Err(LangError::NotImplemented),
            Scheme::App { .. } => Err(LangError::NotImplemented),
            // `handle body with target = impl_` (ADR-0076): eliminate the
            // handler at compile time (rewriting every bare arr(target) in
            // the body to the impl graph), then evaluate the resulting
            // handler-free scheme.
            //
            // ADR-0054 / ADR-0076 effect row tracking: compute the effect row
            // of this handler scheme via [`infer_effect_row`] and attach it as
            // the `effects` field on the resulting function type (if any). The
            // row records which capabilities the scheme manages (the `Handle`
            // targets). Structural inference always produces a closed row.
            Scheme::Handle { .. } => {
                let row = infer_effect_row(scheme);
                let eliminated = handl_language::eliminate_and_check(scheme)
                    .map_err(|e| LangError::engine(e.to_string()))?;
                let result =
                    self.eval_type_scheme(lang, ast, node_ref, &eliminated, ctx, incoming)?;
                // If the result is a type.function, attach the effects field.
                if ctx.registry.as_type_tag(&result) == Some("type.function") {
                    let effects_val = self.effect_row_to_type_value(&row, ctx)?;
                    if let Value::Object(map) = &result {
                        let mut new_map = map.clone();
                        new_map.insert("effects".to_string(), effects_val);
                        return Ok(Value::Object(new_map));
                    }
                }
                Ok(result)
            }
        }
    }

    /// Evaluate a `Native` `Arr` op within a type rule.
    ///
    /// Only `const` and `id` are supported in this initial impl; all other
    /// native ops return an engine error (the ADR notes the op set is
    /// refined during implementation). `Foreign` ops are rejected earlier
    /// in [`Self::eval_type_scheme`] (ADR-0054 pure-eval path).
    fn eval_native_arr(
        &self,
        op: &str,
        lit: Option<&Value>,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        match op {
            "const" => Ok(lit.cloned().unwrap_or(Value::Null)),
            "id" => Ok(incoming.unwrap_or(Value::Null)),
            other => Err(LangError::engine(format!(
                "typecheck: native op '{}' not supported in type rules yet",
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
                let idx = lit.and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: {} requires a u64 lit (child index)",
                        op
                    ))
                })?;
                let child = child_ref(ast, node_ref, idx)?;
                self.synth_node(lang, ast, child, ctx)
            }
            "check_child" => {
                let idx = lit.and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine("typecheck: check_child requires a u64 lit (child index)")
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
                let idx = lit.and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: {} requires a u64 lit (child index)",
                        op
                    ))
                })?;
                let child = child_ref(ast, node_ref, idx)?;
                // ADR-0054 pure-eval path: reject effectful `Foreign` ops in
                // the child's AST subtree before delegating to the eval
                // engine. The host may opt out via `with_pure(false)`.
                if self.pure {
                    check_pure(lang, ast, child)?;
                }
                let sub = ast.subtree(child)?;
                let req = EvalRequest::new(sub);
                self.eval_engine.eval(lang, &req)
            }
            "unify" => {
                if args.len() == 2 {
                    let a = self.eval_type_scheme(
                        lang,
                        ast,
                        node_ref,
                        &args[0],
                        ctx,
                        incoming.clone(),
                    )?;
                    let b = self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                    self.unify(&a, &b, ctx)?;
                } else {
                    let inc = incoming.unwrap_or(Value::Null);
                    let expected = lit.cloned().unwrap_or(Value::Null);
                    self.unify(&inc, &expected, ctx)?;
                }
                Ok(Value::Null)
            }
            "subtype" => {
                // Check if type `a` (sub) is a subtype of type `b` (super).
                // Supports universe cumulativity: `Type_n ⊆ Type_m` if `n ≤ m`
                // (ADR-0054). `args[0]` is the sub-type sub-scheme; `args[1]`
                // is the super-type sub-scheme.
                if args.len() < 2 {
                    return Err(LangError::engine("subtype: requires 2 args (sub, super)"));
                }
                let sub =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let sup = self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                self.check_subtype(&sub, &sup, ctx)?;
                Ok(Value::Null)
            }
            "extend" => {
                // Read the name from `lit` when a non-null literal is present;
                // otherwise read from `incoming` (a string `Value`), mirroring
                // the `lookup` pattern. When the name comes from `incoming`,
                // the type MUST come from `args[0]` (otherwise `incoming`
                // would be used as both the name and the type). When the name
                // comes from `lit`, `args` may be empty (type from `incoming`).
                let (name, name_from_incoming) =
                    match lit.and_then(|v| if v.is_null() { None } else { Some(v) }) {
                        Some(v) => (
                            v.as_str()
                                .ok_or_else(|| {
                                    LangError::engine(
                                        "typecheck: extend: lit must be a string (name)",
                                    )
                                })?
                                .to_string(),
                            false,
                        ),
                        None => (
                            incoming
                                .as_ref()
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    LangError::engine(
                                    "typecheck: extend: name must come from lit or incoming string",
                                )
                                })?
                                .to_string(),
                            true,
                        ),
                    };
                let ty = if args.is_empty() {
                    if name_from_incoming {
                        return Err(LangError::engine(
                            "typecheck: extend: when name comes from incoming, args must be \
                             non-empty (type sub-scheme)",
                        ));
                    }
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
                        LangError::engine("typecheck: lookup: lit must be a string (name)")
                    })?,
                    None => incoming.as_ref().and_then(|v| v.as_str()).ok_or_else(|| {
                        LangError::engine(
                            "typecheck: lookup: requires a string lit or string incoming (name)",
                        )
                    })?,
                };
                ctx.lookup(name).cloned().ok_or_else(|| {
                    LangError::engine(format!("typecheck: lookup: unbound variable '{}'", name))
                })
            }
            "lookup_var" => {
                let idx = lit.and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine("typecheck: lookup_var requires a u64 lit (index)")
                })?;
                ctx.lookup_index(idx as usize).cloned().ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: lookup_var: no binding at index {}",
                        idx
                    ))
                })
            }
            "fresh_meta" => {
                let id = ctx.fresh_meta();
                Ok(meta_value(id))
            }
            "fresh_row_meta" => {
                // ADR-0074 ID.7: allocate a fresh metavariable for use as a
                // row variable in open record types (`type.record{fields:
                // {...}, rest: <row_meta>}`). Functionally identical to
                // `fresh_meta` — the separate op documents intent and leaves
                // room for row-specific constraints in the future.
                let id = ctx.fresh_meta();
                Ok(meta_value(id))
            }
            "solve_meta" => {
                let id = if args.is_empty() {
                    lit.and_then(|v| v.as_u64()).ok_or_else(|| {
                        LangError::engine("typecheck: solve_meta requires a u64 id (lit or arg)")
                    })?
                } else {
                    let id_val = self.eval_type_scheme(
                        lang,
                        ast,
                        node_ref,
                        &args[0],
                        ctx,
                        incoming.clone(),
                    )?;
                    id_val.as_u64().ok_or_else(|| {
                        LangError::engine("typecheck: solve_meta: id arg is not a u64")
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
                let tag = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine("typecheck: construct requires a string lit (tag name)")
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
                        return Err(LangError::engine(format!(
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
                ctx.registry.construct(tag, &payload_val).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: construct: failed to mint type value for tag '{}'",
                        tag
                    ))
                })
            }
            "match_type" => {
                if args.len() < 2 {
                    return Err(LangError::engine(
                        "typecheck: match_type requires 2 args (value, cases)",
                    ));
                }
                let value =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let cases = self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                Ok(ctx
                    .registry
                    .match_type(&value, &cases)
                    .unwrap_or(Value::Null))
            }
            "bind" => {
                let name = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::engine("typecheck: bind requires a string lit (name)")
                    })?
                    .to_string();
                let val = incoming.unwrap_or(Value::Null);
                ctx.bind_result(name, val.clone());
                Ok(val)
            }
            "result" => {
                let name = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine("typecheck: result requires a string lit (name)")
                })?;
                ctx.lookup_result(name).cloned().ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: result: no stored result for '{}'",
                        name
                    ))
                })
            }
            "node_payload" => {
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: node_payload: node index {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                Ok(node.payload.clone())
            }
            "json_type_construct" => {
                // Dispatch on the JSON type of `incoming` (any JSON value,
                // typically from `node_payload`) and mint the appropriate
                // type value via the host type registry.
                let inc = incoming.as_ref().ok_or_else(|| {
                    LangError::engine("typecheck: json_type_construct requires an incoming value")
                })?;
                let tag = match inc {
                    Value::Number(_) => "type.number",
                    Value::String(_) => "type.string",
                    Value::Bool(_) => "type.bool",
                    Value::Null => "type.null",
                    Value::Array(_) => "type.array",
                    Value::Object(_) => "type.object",
                };
                ctx.registry.construct(tag, &Value::Null).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: json_type_construct: failed to mint type value for tag '{}'",
                        tag
                    ))
                })
            }
            "payload_field" => {
                // Extract a field from the current node's payload. `lit` is
                // the field name (string). Returns `Value::Null` if the field
                // is absent. This lets rules extract the `name` field from
                // `let`/`bind` nodes.
                let field = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine("typecheck: payload_field requires a string lit (field name)")
                })?;
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: payload_field: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                Ok(node.payload.get(field).cloned().unwrap_or(Value::Null))
            }
            "field" => {
                // Build a single-field object `{"<field_name>": value}` for
                // use as an arg to `construct`. `lit` is the field name
                // (string); `args[0]` is a sub-scheme producing the value.
                let field_name = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine("typecheck: field requires a string lit (field name)")
                })?;
                if args.is_empty() {
                    return Err(LangError::engine(
                        "typecheck: field requires one arg (value sub-scheme)",
                    ));
                }
                let val =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let mut m = serde_json::Map::new();
                m.insert(field_name.to_string(), val);
                Ok(Value::Object(m))
            }
            "make_array" => {
                // Build a JSON array from the evaluated `args`, in order.
                // Used to construct positional tuple/sum payloads (e.g. the
                // `elements` field of a product type `(A, C)` or the
                // `variants` field of a sum type `(A | B)`) for arrow kind
                // signatures (ADR-0074 follow-up #3). Each arg is a sub-scheme
                // producing one element.
                let mut out = Vec::with_capacity(args.len());
                for arg in args {
                    let val =
                        self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?;
                    out.push(val);
                }
                Ok(Value::Array(out))
            }
            "type_field" => {
                // Extract a field from a type value (a tagged value).
                // `lit` is the field name (string); `args[0]` is a sub-scheme
                // that produces the type value. Returns `Value::Null` if the
                // field is absent. Unlike `payload_field` (which reads from
                // the current AST node's payload), `type_field` operates on
                // an arbitrary type value passed as a sub-scheme arg — this
                // lets rules extract `param`/`body` fields from a
                // `type.function` value (ADR-0054 legacy migration).
                let field_name = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine("typecheck: type_field requires a string lit (field name)")
                })?;
                if args.is_empty() {
                    return Err(LangError::engine(
                        "typecheck: type_field requires one arg (type value sub-scheme)",
                    ));
                }
                let type_val =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                Ok(type_val.get(field_name).cloned().unwrap_or(Value::Null))
            }
            "synth_last_child" => {
                // Synthesize the type of the last child of the current node.
                // If the node has no children (empty block), returns
                // `type.unit`. This supports `block` type rules where the
                // block's type is the type of its last statement
                // (ADR-0054 legacy migration).
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: synth_last_child: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "typecheck: synth_last_child: node has no children array",
                            node,
                        )
                    })?;
                if children.is_empty() {
                    // Empty block → type.unit
                    return ctx
                        .registry
                        .construct("type.unit", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "typecheck: construct failed for type.unit",
                                node,
                            )
                        });
                }
                let last_idx = children.last().and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine_node(
                        "typecheck: synth_last_child: last child index is not a u64",
                        node,
                    )
                })?;
                let child = AstRef::new(last_idx as usize);
                self.synth_node(lang, ast, child, ctx)
            }
            "synth_block" => {
                // Synthesize the type of a graph block (ADR-0079). Iterates
                // through the block's children: each `assign` child binds a
                // local variable in Γ (synthing its value type, or unifying
                // with a type annotation if present); non-assign children are
                // synthed for effect. Returns the type of the last child. An
                // empty block returns `type.unit`.
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: synth_block: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "typecheck: synth_block: node has no children array",
                            node,
                        )
                    })?;
                if children.is_empty() {
                    return ctx
                        .registry
                        .construct("type.unit", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine(
                                "typecheck: synth_block: construct failed for type.unit",
                            )
                        });
                }
                let last_idx = children.last().and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine_node(
                        "typecheck: synth_block: last child index is not a u64",
                        node,
                    )
                })? as usize;
                // Bind each assign child (all children except the last, which
                // is the block's result expression).
                for (i, entry) in children.iter().enumerate() {
                    if i == children.len() - 1 {
                        break;
                    }
                    let child_idx = entry.as_u64().ok_or_else(|| {
                        LangError::engine_node(
                            "typecheck: synth_block: child index is not a u64",
                            node,
                        )
                    })? as usize;
                    let child_node = ast.get(AstRef::new(child_idx)).ok_or_else(|| {
                        LangError::engine(format!(
                            "typecheck: synth_block: child node {} out of bounds",
                            child_idx
                        ))
                    })?;
                    if child_node.kind != "assign" {
                        // Non-assign child: synth for effect (type errors still
                        // surface). The result is discarded.
                        let _ = self.synth_node(lang, ast, AstRef::new(child_idx), ctx)?;
                        continue;
                    }
                    // Extract (name, value_ref, optional annotation) from the
                    // assign node. Supports both v0.1 (payload.name + child 0)
                    // and legacy (children [var|type-annot, value]) shapes.
                    let assign_children = child_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                        .ok_or_else(|| {
                            LangError::engine(
                                "typecheck: synth_block: assign has no children array",
                            )
                        })?;
                    let (name, value_ref, ann_ref) = if let Some(n) =
                        child_node.payload.get("name").and_then(|v| v.as_str())
                    {
                        // v0.1: name in payload, value is child 0.
                        if assign_children.is_empty() {
                            return Err(LangError::engine(
                                    "typecheck: synth_block: assign with payload.name needs a value child",
                                ));
                        }
                        let vidx = assign_children[0].as_u64().ok_or_else(|| {
                            LangError::engine(
                                "typecheck: synth_block: assign value index is not a u64",
                            )
                        })? as usize;
                        (n.to_string(), AstRef::new(vidx), None)
                    } else {
                        // Legacy: children [lhs, value] where lhs is a
                        // `var` or a `type-annot`.
                        if assign_children.len() < 2 {
                            return Err(LangError::engine(
                                "typecheck: synth_block: assign needs at least 2 children",
                            ));
                        }
                        let lhs_idx = assign_children[0].as_u64().ok_or_else(|| {
                            LangError::engine(
                                "typecheck: synth_block: assign lhs index is not a u64",
                            )
                        })? as usize;
                        let vidx = assign_children[1].as_u64().ok_or_else(|| {
                            LangError::engine(
                                "typecheck: synth_block: assign value index is not a u64",
                            )
                        })? as usize;
                        let lhs_node = ast.get(AstRef::new(lhs_idx)).ok_or_else(|| {
                            LangError::engine(format!(
                                "typecheck: synth_block: lhs node {} out of bounds",
                                lhs_idx
                            ))
                        })?;
                        if is_type_annot(lang, &lhs_node) {
                            let ta_children = lhs_node
                                .payload
                                .get("children")
                                .and_then(|c| c.as_array())
                                .ok_or_else(|| {
                                    LangError::engine(
                                        "typecheck: synth_block: type-annot has no children",
                                    )
                                })?;
                            if ta_children.len() < 2 {
                                return Err(LangError::engine(
                                    "typecheck: synth_block: type-annot needs 2 children",
                                ));
                            }
                            let inner_var_idx = ta_children[0].as_u64().ok_or_else(|| {
                                LangError::engine(
                                    "typecheck: synth_block: type-annot var index is not a u64",
                                )
                            })? as usize;
                            let type_expr_idx = ta_children[1].as_u64().ok_or_else(|| {
                                LangError::engine(
                                    "typecheck: synth_block: type-annot type index is not a u64",
                                )
                            })? as usize;
                            let inner_var =
                                ast.get(AstRef::new(inner_var_idx)).ok_or_else(|| {
                                    LangError::engine(format!(
                                        "typecheck: synth_block: inner var node {} out of bounds",
                                        inner_var_idx
                                    ))
                                })?;
                            let n = inner_var.payload.as_str().ok_or_else(|| {
                                LangError::engine(
                                    "typecheck: synth_block: inner var payload is not a string",
                                )
                            })?;
                            (
                                n.to_string(),
                                AstRef::new(vidx),
                                Some(AstRef::new(type_expr_idx)),
                            )
                        } else {
                            let n = lhs_node.payload.as_str().ok_or_else(|| {
                                LangError::engine(
                                    "typecheck: synth_block: var payload is not a string",
                                )
                            })?;
                            (n.to_string(), AstRef::new(vidx), None)
                        }
                    };
                    let value_type = self.synth_node(lang, ast, value_ref, ctx)?;
                    let bound_type = if let Some(ann_ref) = ann_ref {
                        let type_expr_node = ast.get(ann_ref).ok_or_else(|| {
                            LangError::engine(format!(
                                "typecheck: synth_block: type-expr node {} out of bounds",
                                ann_ref.index()
                            ))
                        })?;
                        let ann = interpret_type_expr(lang, ast, type_expr_node, &ctx.registry)?;
                        self.unify(&value_type, &ann, ctx)?;
                        ann
                    } else {
                        value_type
                    };
                    ctx.extend(name, bound_type);
                }
                self.synth_node(lang, ast, AstRef::new(last_idx), ctx)
            }
            "check_json_value" => {
                // Check the current node's JSON payload against an expected
                // collection type (type.number, type.string, type.bool,
                // type.null). The expected type is in `incoming` (threaded by
                // the `check` scheme) or `args[0]`. Returns `Null` on success,
                // errors on mismatch (ADR-0054 directed checking for `value`).
                let expected = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let expected_tag = ctx.registry.as_type_tag(&expected).ok_or_else(|| {
                    LangError::engine("check_json_value: expected type is not a tagged type value")
                })?;
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "check_json_value: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let payload = &node.payload;
                let matches = match expected_tag {
                    "type.number" => payload.is_number(),
                    "type.string" => payload.is_string(),
                    "type.bool" => payload.is_boolean(),
                    "type.null" => payload.is_null(),
                    _ => false,
                };
                if matches {
                    Ok(Value::Null)
                } else {
                    Err(LangError::engine(format!(
                        "typecheck: value {:?} does not match expected type {}",
                        payload, expected_tag
                    )))
                }
            }
            "check_last_child" => {
                // Check the last child of the current node against the expected
                // type (incoming or args[0]). Supports directed checking for
                // `block` nodes: the block's type is the type of its last
                // statement (ADR-0054).
                let expected = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "check_last_child: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node("check_last_child: node has no children array", node)
                    })?;
                if children.is_empty() {
                    // Empty block — accept any expected type (the block is
                    // vacuously well-typed; a precise check would unify with
                    // type.unit).
                    return Ok(Value::Null);
                }
                let last_idx = children.last().and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine_node("check_last_child: last child index is not a u64", node)
                })?;
                let child = AstRef::new(last_idx as usize);
                self.check_node(lang, ast, child, &expected, ctx)?;
                Ok(Value::Null)
            }
            "check_block" => {
                // Directed checking for a graph block (ADR-0079). Binds
                // assign children in Γ (same as `synth_block`), then checks
                // the last child against the expected type. An empty block
                // is vacuously well-typed.
                let expected = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "check_block: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node("check_block: node has no children array", node)
                    })?;
                if children.is_empty() {
                    return Ok(Value::Null);
                }
                let last_idx = children.last().and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine_node("check_block: last child index is not a u64", node)
                })? as usize;
                // Bind each assign child (all children except the last).
                for (i, entry) in children.iter().enumerate() {
                    if i == children.len() - 1 {
                        break;
                    }
                    let child_idx = entry.as_u64().ok_or_else(|| {
                        LangError::engine_node("check_block: child index is not a u64", node)
                    })? as usize;
                    let child_node = ast.get(AstRef::new(child_idx)).ok_or_else(|| {
                        LangError::engine(format!(
                            "check_block: child node {} out of bounds",
                            child_idx
                        ))
                    })?;
                    if child_node.kind != "assign" {
                        let _ = self.synth_node(lang, ast, AstRef::new(child_idx), ctx)?;
                        continue;
                    }
                    let assign_children = child_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "check_block: assign has no children array",
                                node,
                            )
                        })?;
                    let (name, value_ref, ann_ref) =
                        if let Some(n) = child_node.payload.get("name").and_then(|v| v.as_str()) {
                            if assign_children.is_empty() {
                                return Err(LangError::engine(
                                    "check_block: assign with payload.name needs a value child",
                                ));
                            }
                            let vidx = assign_children[0].as_u64().ok_or_else(|| {
                                LangError::engine_node(
                                    "check_block: assign value index is not a u64",
                                    node,
                                )
                            })? as usize;
                            (n.to_string(), AstRef::new(vidx), None)
                        } else {
                            if assign_children.len() < 2 {
                                return Err(LangError::engine(
                                    "check_block: assign needs at least 2 children",
                                ));
                            }
                            let lhs_idx = assign_children[0].as_u64().ok_or_else(|| {
                                LangError::engine_node(
                                    "check_block: assign lhs index is not a u64",
                                    node,
                                )
                            })? as usize;
                            let vidx = assign_children[1].as_u64().ok_or_else(|| {
                                LangError::engine_node(
                                    "check_block: assign value index is not a u64",
                                    node,
                                )
                            })? as usize;
                            let lhs_node = ast.get(AstRef::new(lhs_idx)).ok_or_else(|| {
                                LangError::engine(format!(
                                    "check_block: lhs node {} out of bounds",
                                    lhs_idx
                                ))
                            })?;
                            if is_type_annot(lang, &lhs_node) {
                                let ta_children = lhs_node
                                    .payload
                                    .get("children")
                                    .and_then(|c| c.as_array())
                                    .ok_or_else(|| {
                                        LangError::engine_node(
                                            "check_block: type-annot has no children",
                                            node,
                                        )
                                    })?;
                                if ta_children.len() < 2 {
                                    return Err(LangError::engine(
                                        "check_block: type-annot needs 2 children",
                                    ));
                                }
                                let inner_var_idx = ta_children[0].as_u64().ok_or_else(|| {
                                    LangError::engine_node(
                                        "check_block: type-annot var index is not a u64",
                                        node,
                                    )
                                })? as usize;
                                let type_expr_idx = ta_children[1].as_u64().ok_or_else(|| {
                                    LangError::engine_node(
                                        "check_block: type-annot type index is not a u64",
                                        node,
                                    )
                                })? as usize;
                                let inner_var =
                                    ast.get(AstRef::new(inner_var_idx)).ok_or_else(|| {
                                        LangError::engine(format!(
                                            "check_block: inner var node {} out of bounds",
                                            inner_var_idx
                                        ))
                                    })?;
                                let n = inner_var.payload.as_str().ok_or_else(|| {
                                    LangError::engine_node(
                                        "check_block: inner var payload is not a string",
                                        node,
                                    )
                                })?;
                                (
                                    n.to_string(),
                                    AstRef::new(vidx),
                                    Some(AstRef::new(type_expr_idx)),
                                )
                            } else {
                                let n = lhs_node.payload.as_str().ok_or_else(|| {
                                    LangError::engine_node(
                                        "check_block: var payload is not a string",
                                        node,
                                    )
                                })?;
                                (n.to_string(), AstRef::new(vidx), None)
                            }
                        };
                    let value_type = self.synth_node(lang, ast, value_ref, ctx)?;
                    let bound_type = if let Some(ann_ref) = ann_ref {
                        let type_expr_node = ast.get(ann_ref).ok_or_else(|| {
                            LangError::engine(format!(
                                "check_block: type-expr node {} out of bounds",
                                ann_ref.index()
                            ))
                        })?;
                        let ann = interpret_type_expr(lang, ast, type_expr_node, &ctx.registry)?;
                        self.unify(&value_type, &ann, ctx)?;
                        ann
                    } else {
                        value_type
                    };
                    ctx.extend(name, bound_type);
                }
                self.check_node(lang, ast, AstRef::new(last_idx), &expected, ctx)?;
                Ok(Value::Null)
            }
            "check_all_children" => {
                // Check all children of the current node against a single
                // expected type (useful for checking array elements against a
                // uniform element type). The expected type comes from
                // `incoming` (threaded by the `check` scheme) or `args[0]`
                // (ADR-0054 — Scheme lacks iteration, so this is a judgment op).
                let expected = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "check_all_children: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "check_all_children: node has no children array",
                            node,
                        )
                    })?;
                for child_entry in children {
                    let child_idx = child_entry.as_u64().ok_or_else(|| {
                        LangError::engine_node("check_all_children: child index is not a u64", node)
                    })? as usize;
                    let child_ref = AstRef::new(child_idx);
                    self.check_node(lang, ast, child_ref, &expected, ctx)?;
                }
                Ok(Value::Null)
            }
            "synth_match_arms" => {
                // Multi-arm unification for `match` nodes. The match node's
                // children are: [scrutinee, block] where the block's children
                // are the arm nodes. Each arm is a node with children
                // [pattern, body]. We synth the body of each arm (child 1 of
                // each arm node) and unify them all, returning the unified
                // type. With fewer than 2 children (no arms), returns
                // `type.unknown`
                // (ADR-0054 — Scheme lacks iteration, so this is a judgment op).
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "synth_match_arms: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node("synth_match_arms: node has no children array", node)
                    })?;
                if children.len() < 2 {
                    // No arms — return type.unknown
                    return ctx
                        .registry
                        .construct("type.unknown", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "synth_match_arms: construct failed for type.unknown",
                                node,
                            )
                        });
                }
                // Child 1 is a `block` node whose children are the arms.
                // If it is a block, iterate its children; otherwise iterate
                // match's children 1.. directly (legacy form).
                let block_idx = children[1].as_u64().ok_or_else(|| {
                    LangError::engine_node("synth_match_arms: arm child index is not a u64", node)
                })? as usize;
                let block_ref = AstRef::new(block_idx);
                let arm_indices: Vec<usize> = {
                    let block_node = ast.get(block_ref).ok_or_else(|| {
                        LangError::engine(format!(
                            "synth_match_arms: block node {} out of bounds",
                            block_idx
                        ))
                    })?;
                    if is_block_node(lang, &block_node) {
                        block_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::engine(
                                    "synth_match_arms: block node has no children array",
                                )
                            })?
                            .iter()
                            .map(|v| {
                                v.as_u64().ok_or_else(|| {
                                    LangError::engine_node(
                                        "synth_match_arms: arm index is not a u64",
                                        node,
                                    )
                                }) as Result<u64, LangError>
                            })
                            .map(|r| r.map(|v| v as usize))
                            .collect::<Result<Vec<_>, _>>()?
                    } else {
                        // Legacy: arms are direct children of match (1..)
                        children[1..]
                            .iter()
                            .map(|v| {
                                v.as_u64().ok_or_else(|| {
                                    LangError::engine_node(
                                        "synth_match_arms: arm index is not a u64",
                                        node,
                                    )
                                }) as Result<u64, LangError>
                            })
                            .map(|r| r.map(|v| v as usize))
                            .collect::<Result<Vec<_>, _>>()?
                    }
                };
                let mut arm_types: Vec<Value> = Vec::new();
                for arm_idx in arm_indices {
                    let arm_ref = AstRef::new(arm_idx);
                    let arm_node = ast.get(arm_ref).ok_or_else(|| {
                        LangError::engine(format!(
                            "synth_match_arms: arm node {} out of bounds",
                            arm_idx
                        ))
                    })?;
                    // The arm node has children [pattern, body]
                    let arm_children = arm_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "synth_match_arms: arm node has no children array",
                                node,
                            )
                        })?;
                    if arm_children.len() < 2 {
                        return Err(LangError::engine(
                            "synth_match_arms: arm node needs at least 2 children (pattern, body)",
                        ));
                    }
                    let body_idx = arm_children[1].as_u64().ok_or_else(|| {
                        LangError::engine_node(
                            "synth_match_arms: body child index is not a u64",
                            node,
                        )
                    })? as usize;
                    let body_ref = AstRef::new(body_idx);
                    let body_type = self.synth_node(lang, ast, body_ref, ctx)?;
                    arm_types.push(body_type);
                }
                // Unify all arm types
                if arm_types.is_empty() {
                    return ctx
                        .registry
                        .construct("type.unknown", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "synth_match_arms: construct failed for type.unknown",
                                node,
                            )
                        });
                }
                let unified = arm_types[0].clone();
                for arm_type in &arm_types[1..] {
                    self.unify(&unified, arm_type, ctx)?;
                    // After unification, the unified type might have been
                    // refined by meta solving; for simplicity, keep the first
                    // type (metas are solved in-place).
                }
                Ok(unified)
            }
            "synth_rec" => {
                // Typecheck a `rec` region (ADR-0074 ID.3 — Paterson loop
                // fixpoint typing). The `rec` node is a flattened block: its
                // children are bindings (`assign` nodes) and expressions.
                // Bindings may reference each other cyclically. The judgment
                // allocates a fresh metavariable per binding, extends Γ, and
                // iterates synth+unify until the binding types stabilize
                // (the fixpoint). A fuel limit prevents divergence. The
                // result is the type of the last child (or a `return` child's
                // value). Termination is NOT checked at the type level.
                self.synth_rec_judgment(lang, ast, node_ref, ctx)
            }
            // ADR-0079: `synth_let` removed (let keyword deleted, graph blocks
            // replace it). The `synth_block` handler handles assign children.
            "synth_function" => {
                // Typecheck a `function` node. The node has children
                // [paren, body] where `paren` has children [var(name)]. We
                // bind the parameter name to a fresh metavariable (unknown
                // type) in Γ, synth the body, then construct
                // type.function{param: meta, body: body-type}
                // (ADR-0054 — the DSL can't navigate grandchildren or create
                // metas, so this is a judgment op).
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "synth_function: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node("synth_function: node has no children array", node)
                    })?;
                if children.len() < 2 {
                    return Err(LangError::engine(
                        "synth_function: node needs at least 2 children (params, body)",
                    ));
                }
                // Resolve params/body by child role (ADR-0081), falling back
                // to the positional children for role-less shapes.
                let params_ref = role_child_ref(lang, node, "params").or_else(|| {
                    children
                        .first()
                        .and_then(|v| v.as_u64())
                        .map(|i| AstRef::new(i as usize))
                });
                let body_ref = role_child_ref(lang, node, "body").or_else(|| {
                    children
                        .get(1)
                        .and_then(|v| v.as_u64())
                        .map(|i| AstRef::new(i as usize))
                });
                let params_ref = params_ref.ok_or_else(|| {
                    LangError::engine_node("synth_function: no params child", node)
                })?;
                let body_ref = body_ref
                    .ok_or_else(|| LangError::engine_node("synth_function: no body child", node))?;
                let params_node = ast.get(params_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "synth_function: params node {} out of bounds",
                        params_ref.index()
                    ))
                })?;
                // Unwrap the params wrapper (paren{body}) by role, falling
                // back to its first child; a bare var params node carries the
                // name directly in its payload.
                let param_name = if is_paren_node(lang, &params_node) {
                    let var_ref = role_child_ref(lang, &params_node, "body")
                        .or_else(|| {
                            params_node
                                .payload
                                .get("children")?
                                .as_array()?
                                .first()
                                .and_then(|v| v.as_u64())
                                .map(|i| AstRef::new(i as usize))
                        })
                        .ok_or_else(|| {
                            LangError::engine("synth_function: paren node has no param children")
                        })?;
                    let var_node = ast.get(var_ref).ok_or_else(|| {
                        LangError::engine(format!(
                            "synth_function: var node {} out of bounds",
                            var_ref.index()
                        ))
                    })?;
                    var_node.payload.as_str().ok_or_else(|| {
                        LangError::engine_node(
                            "synth_function: var node payload is not a string",
                            node,
                        )
                    })?
                } else if params_node.kind == "var" {
                    params_node.payload.as_str().ok_or_else(|| {
                        LangError::engine_node(
                            "synth_function: var node payload is not a string",
                            node,
                        )
                    })?
                } else {
                    return Err(LangError::engine(format!(
                        "synth_function: expected paren or var for params, got '{}'",
                        params_node.kind
                    )));
                };
                let param_meta = ctx.fresh_meta();
                let param_type = meta_value(param_meta);
                ctx.extend(param_name.to_string(), param_type.clone());
                let body_type = self.synth_node(lang, ast, body_ref, ctx)?;
                let payload = serde_json::json!({"param": param_type, "body": body_type});
                ctx.registry
                    .construct("type.function", &payload)
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "synth_function: construct failed for type.function",
                            node,
                        )
                    })
            }
            "synth_type_annot" => {
                // Typecheck a `type-annot` node (`expr: T`). Child 0 is the
                // annotated expression, child 1 is the type expression (e.g.
                // `type.number` parses as `field-access(var("type"), "number")`).
                //
                // ADR-0068 step B1: If the type expression evaluates to a
                // predicate (a closure returning `type.bool`), normalize
                // `T(e)` via the bound eval engine. `Proven` → accept
                // statically (no runtime check); `Disproved` → reject;
                // `Blocked` → accept with a runtime backstop. Otherwise, fall
                // back to the existing synth-then-unify path.
                //
                // This branch is data-driven: it inspects the *value's* shape
                // (type tag / closure marker), not the AST node kind, per the
                // engine-genericity rule.
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "synth_type_annot: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node("synth_type_annot: node has no children array", node)
                    })?;
                if children.len() < 2 {
                    return Err(LangError::engine(
                        "synth_type_annot: node needs 2 children (expr, type)",
                    ));
                }
                let expr_idx = children[0].as_u64().ok_or_else(|| {
                    LangError::engine_node("synth_type_annot: expr child index is not a u64", node)
                })? as usize;
                let type_idx = children[1].as_u64().ok_or_else(|| {
                    LangError::engine_node("synth_type_annot: type child index is not a u64", node)
                })? as usize;
                let expr_type = self.synth_node(lang, ast, AstRef::new(expr_idx), ctx)?;
                let type_node = ast.get(AstRef::new(type_idx)).ok_or_else(|| {
                    LangError::engine(format!(
                        "synth_type_annot: type node {} out of bounds",
                        type_idx
                    ))
                })?;

                // Try the existing type-syntax path first (handles
                // `type.number`, `(type.number) -> type.bool`, etc.).
                let annotation_result =
                    interpret_type_expr(lang, ast, type_node, &ctx.registry);

                match annotation_result {
                    Ok(annotation_type) => {
                        // Phase 7: Check for meta-predicate types (Total, Pure).
                        // These are not regular predicate functions — they
                        // assert a meta-property of the function itself
                        // (termination, purity) and require special handling.
                        let annotation_tag = ctx.registry.as_type_tag(&annotation_type);
                        match annotation_tag {
                            Some("type.Total") => {
                                // Total predicate: check structural recursion.
                                let proof = totality_check::check_totality(
                                    ast,
                                    AstRef::new(expr_idx),
                                    lang,
                                );
                                return self.resolve_predicate_proof(
                                    proof,
                                    &expr_type,
                                    &annotation_type,
                                    ctx,
                                );
                            }
                            Some("type.Pure") => {
                                // Pure predicate: check effect row emptiness.
                                let proof = self.check_purity(&expr_type);
                                return self.resolve_predicate_proof(
                                    proof,
                                    &expr_type,
                                    &annotation_type,
                                    ctx,
                                );
                            }
                            _ => {}
                        }

                        // If the annotation is a `type.function` whose body
                        // is `type.bool`, it is a predicate type: evaluate
                        // the type expression to get the predicate closure
                        // and apply it to the expression value.
                        let body_is_bool = annotation_type
                            .get("body")
                            .map(|b| ctx.registry.as_type_tag(b) == Some("type.bool"))
                            .unwrap_or(false);
                        let is_predicate =
                            ctx.registry.as_type_tag(&annotation_type) == Some("type.function")
                                && body_is_bool;

                        if is_predicate {
                            let proof = self.check_predicate_application(
                                lang,
                                ast,
                                AstRef::new(type_idx),
                                AstRef::new(expr_idx),
                                100_000,
                            );
                            // B2: if B1 (concrete eval) is blocked, try
                            // symbolic normalization — the expression may
                            // have free variables that B1 can't handle but
                            // B2 can reason about symbolically.
                            let proof = if proof.verdict == Verdict::Blocked {
                                // The type expression is a `function` node
                                // (predicate). Find it in the AST to pass
                                // to the symbolic checker.
                                let sym_proof = self.check_predicate_symbolic(
                                    lang,
                                    ast,
                                    AstRef::new(type_idx),
                                    AstRef::new(expr_idx),
                                    100_000,
                                );
                                // Only use the symbolic result if it's
                                // better (Proven or Disproved); otherwise
                                // keep the original Blocked.
                                if sym_proof.verdict != Verdict::Blocked {
                                    sym_proof
                                } else {
                                    proof
                                }
                            } else {
                                proof
                            };
                            return self.resolve_predicate_proof(
                                proof,
                                &expr_type,
                                &annotation_type,
                                ctx,
                            );
                        }

                        // Not a predicate — use the existing unify path.
                        self.unify(&expr_type, &annotation_type, ctx)?;
                        Ok(annotation_type)
                    }
                    Err(interp_err) => {
                        // `interpret_type_expr` failed — the type expression
                        // might be a value expression (e.g. `DivisibleBy(3)`
                        // or `(n) -> n % 3 == 0`). Try evaluating it via the
                        // eval engine to get a predicate closure.
                        if self.pure {
                            if let Err(_e) = check_pure(lang, ast, AstRef::new(type_idx)) {
                                // Impure type expression — can't evaluate
                                // safely. Return the original
                                // `interpret_type_expr` error.
                                return Err(interp_err);
                            }
                        }
                        let sub = ast.subtree(AstRef::new(type_idx))?;
                        let req = EvalRequest::new(sub);
                        match self.eval_engine.eval(lang, &req) {
                            Ok(type_val) if Self::is_closure_value(&type_val) => {
                                // It's a predicate closure. Apply it to the
                                // expression value.
                                let proof = self.check_predicate_application(
                                    lang,
                                    ast,
                                    AstRef::new(type_idx),
                                    AstRef::new(expr_idx),
                                    100_000,
                                );
                                // B2: if B1 is blocked, try symbolic
                                // normalization.
                                let proof = if proof.verdict == Verdict::Blocked {
                                    let sym_proof = self.check_predicate_symbolic(
                                        lang,
                                        ast,
                                        AstRef::new(type_idx),
                                        AstRef::new(expr_idx),
                                        100_000,
                                    );
                                    if sym_proof.verdict != Verdict::Blocked {
                                        sym_proof
                                    } else {
                                        proof
                                    }
                                } else {
                                    proof
                                };
                                // The annotation type for a closure-valued
                                // predicate is the expression's own type
                                // (the predicate is a runtime contract, not
                                // a static type tag).
                                self.resolve_predicate_proof(proof, &expr_type, &expr_type, ctx)
                            }
                            Ok(_) => Err(interp_err),
                            Err(_) => Err(interp_err),
                        }
                    }
                }
            }
            "synth_first_child_or_unknown" => {
                // Synthesize the type of the first child of the current node.
                // If the node has no children (empty array), returns
                // `type.unknown`. This supports `array` type rules where the
                // element type is the type of the first element, with a
                // fallback for empty arrays.
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: synth_first_child_or_unknown: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let children = node.payload.get("children").and_then(|c| c.as_array());
                match children {
                    Some(arr) if !arr.is_empty() => {
                        let first_idx = arr.first().and_then(|v| v.as_u64()).ok_or_else(|| {
                            LangError::engine(
                                "typecheck: synth_first_child_or_unknown: first child index is not a u64",
                            )
                        })?;
                        let child = AstRef::new(first_idx as usize);
                        self.synth_node(lang, ast, child, ctx)
                    }
                    _ => {
                        // Empty or no children → type.unknown
                        ctx.registry
                            .construct("type.unknown", &Value::Null)
                            .ok_or_else(|| {
                                LangError::engine(
                                    "typecheck: synth_first_child_or_unknown: construct failed for type.unknown",
                                )
                            })
                    }
                }
            }
            "record_field_type" => {
                // Extract a field's type from a `type.record` value. The field
                // name comes from `lit` (string) or `incoming` (string); the
                // record type value comes from `args[0]` (a sub-scheme that
                // evaluates to the record type value).
                let field_name = match lit.and_then(|v| if v.is_null() { None } else { Some(v) }) {
                    Some(v) => v
                        .as_str()
                        .ok_or_else(|| {
                            LangError::engine("record_field_type: lit must be a string field name")
                        })?
                        .to_string(),
                    None => incoming
                        .as_ref()
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            LangError::engine("record_field_type: no field name in lit or incoming")
                        })?
                        .to_string(),
                };
                if args.is_empty() {
                    return Err(LangError::engine(
                        "record_field_type: requires args[0] (record type sub-scheme)",
                    ));
                }
                let record_type =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let fields = record_type
                    .get("fields")
                    .and_then(|f| f.as_object())
                    .ok_or_else(|| {
                        LangError::engine(format!(
                            "record_field_type: type value has no 'fields' object: {:?}",
                            record_type
                        ))
                    })?;
                let field_type = fields.get(&field_name).ok_or_else(|| {
                    LangError::engine(format!(
                        "record_field_type: field '{}' not found in record type",
                        field_name
                    ))
                })?;
                Ok(field_type.clone())
            }
            "array_element_type" => {
                // Extract the element type from a `type.array` value. The
                // array type value comes from `args[0]` (a sub-scheme) or
                // `incoming` (when no args are provided).
                let array_type = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let element_type = array_type.get("element").ok_or_else(|| {
                    LangError::engine(format!(
                        "array_element_type: type value has no 'element' field: {:?}",
                        array_type
                    ))
                })?;
                Ok(element_type.clone())
            }
            "synth_object_record" => {
                // Synthesize a `type.record{fields: {...}}` from an `object`
                // node's `entries` array. Each entry has a `key` (string) and
                // a `value` (JSON value); the value's JSON type becomes the
                // field type (ADR-0054 — `object` uses `entries`, not
                // `children`, so a dedicated op iterates the entries).
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "synth_object_record: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let entries = node
                    .payload
                    .get("entries")
                    .and_then(|e| e.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "synth_object_record: node has no entries array",
                            node,
                        )
                    })?;
                let mut fields = serde_json::Map::new();
                for entry in entries {
                    let key = entry.get("key").and_then(|k| k.as_str()).ok_or_else(|| {
                        LangError::engine_node(
                            "synth_object_record: entry missing 'key' string",
                            node,
                        )
                    })?;
                    let value = entry.get("value").ok_or_else(|| {
                        LangError::engine_node("synth_object_record: entry missing 'value'", node)
                    })?;
                    let tag = match value {
                        Value::Number(_) => "type.number",
                        Value::String(_) => "type.string",
                        Value::Bool(_) => "type.bool",
                        Value::Null => "type.null",
                        Value::Array(_) => "type.array",
                        Value::Object(_) => "type.object",
                    };
                    let field_type =
                        ctx.registry.construct(tag, &Value::Null).ok_or_else(|| {
                            LangError::engine(format!(
                                "synth_object_record: construct failed for tag '{}'",
                                tag
                            ))
                        })?;
                    fields.insert(key.to_string(), field_type);
                }
                let fields_value = Value::Object(fields);
                let mut payload = serde_json::Map::new();
                payload.insert("fields".to_string(), fields_value);
                ctx.registry
                    .construct("type.record", &Value::Object(payload))
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "synth_object_record: construct failed for type.record",
                            node,
                        )
                    })
            }
            "check_object_record" => {
                // Check an `object` node's entries against an expected
                // `type.record{fields: {...}}`. For each entry, look up the
                // field type in the expected record and verify the entry's
                // JSON value matches that type (ADR-0054 — directed checking
                // for `object` against a known record type).
                //
                // ADR-0074 ID.7 — row polymorphism: when the expected
                // `type.record` has a `rest` field (row variable), entries
                // with keys NOT in the expected `fields` are collected as
                // extra fields and assigned to the `rest` row variable. When
                // `rest` is absent (closed record), every entry must have a
                // matching field in the expected record (the original
                // behavior).
                let expected = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let expected_fields = expected
                    .get("fields")
                    .and_then(|f| f.as_object())
                    .ok_or_else(|| {
                        LangError::engine(format!(
                            "check_object_record: expected type has no 'fields' object: {:?}",
                            expected
                        ))
                    })?;
                let rest = expected.get("rest");
                let node = ast.get(node_ref).ok_or_else(|| {
                    LangError::engine(format!(
                        "check_object_record: node {} out of bounds",
                        node_ref.index()
                    ))
                })?;
                let entries = node
                    .payload
                    .get("entries")
                    .and_then(|e| e.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "check_object_record: node has no entries array",
                            node,
                        )
                    })?;
                // Collect extra fields (entries not in expected_fields) for
                // the row variable when the expected record is open.
                let mut extra_fields: serde_json::Map<String, Value> = serde_json::Map::new();
                for entry in entries {
                    let key = entry.get("key").and_then(|k| k.as_str()).ok_or_else(|| {
                        LangError::engine_node(
                            "check_object_record: entry missing 'key' string",
                            node,
                        )
                    })?;
                    let value = entry.get("value").ok_or_else(|| {
                        LangError::engine_node("check_object_record: entry missing 'value'", node)
                    })?;
                    let field_type = expected_fields.get(key);
                    match field_type {
                        Some(ft) => {
                            let expected_tag = ctx.registry.as_type_tag(ft).ok_or_else(|| {
                                LangError::engine(format!(
                                    "check_object_record: field '{}' type is not a tagged type value",
                                    key
                                ))
                            })?;
                            let matches = match expected_tag {
                                "type.number" => value.is_number(),
                                "type.string" => value.is_string(),
                                "type.bool" => value.is_boolean(),
                                "type.null" => value.is_null(),
                                "type.array" => value.is_array(),
                                "type.object" => value.is_object(),
                                _ => false,
                            };
                            if !matches {
                                return Err(LangError::engine(format!(
                                    "check_object_record: field '{}' value {:?} does not match expected type {}",
                                    key, value, expected_tag
                                )));
                            }
                        }
                        None => {
                            // Entry key not in expected fields.
                            if let Some(_rest_val) = rest {
                                // Open record: collect as an extra field for
                                // the row variable. Infer the field type from
                                // the JSON value's type.
                                let tag = match value {
                                    Value::Number(_) => "type.number",
                                    Value::String(_) => "type.string",
                                    Value::Bool(_) => "type.bool",
                                    Value::Null => "type.null",
                                    Value::Array(_) => "type.array",
                                    Value::Object(_) => "type.object",
                                };
                                let field_ty =
                                    ctx.registry.construct(tag, &Value::Null).ok_or_else(|| {
                                        LangError::engine(format!(
                                            "check_object_record: construct failed for tag '{}'",
                                            tag
                                        ))
                                    })?;
                                extra_fields.insert(key.to_string(), field_ty);
                            } else {
                                // Closed record: no extra fields allowed.
                                return Err(LangError::engine(format!(
                                    "check_object_record: field '{}' not found in expected record",
                                    key
                                )));
                            }
                        }
                    }
                }
                // If the expected record is open, assign the collected extra
                // fields to the row variable (`rest`).
                if let Some(rest_val) = rest {
                    let extra_record = self.construct_record(&extra_fields, None, ctx)?;
                    self.unify(rest_val, &extra_record, ctx)?;
                }
                Ok(Value::Null)
            }
            // --- ADR-0054 dependent-type closure machinery (Pi/Sigma) ---
            //
            // These ops bring NBE closure machinery into the data-driven
            // engine for dependent type support. A Pi/Sigma type value carries
            // a `body_closure` field (instead of a flat `body` field) that
            // records the variable name and the AST child index of the body.
            // When the closure is applied, the engine marks Γ, extends it with
            // the variable, synths the body child, and restores Γ — mirroring
            // the legacy `Closure::apply` (ADR-0054).
            "construct_pi" => {
                // Construct a dependent function type `Π (x: param) → body(x)`.
                // `lit` is the variable name (default "x"); `args[0]` is a
                // sub-scheme producing the param type. The body child is child
                // 1 of the current node; its arena index is stored in the
                // closure marker for lazy evaluation via `pi_apply`.
                let var_name = lit.and_then(|v| v.as_str()).unwrap_or("x").to_string();
                let param = if args.is_empty() {
                    incoming.clone().unwrap_or(Value::Null)
                } else {
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?
                };
                let body_child = child_ref(ast, node_ref, 1)?;
                let body_child_idx = body_child.index();
                let mut payload = serde_json::Map::new();
                payload.insert("param".to_string(), param);
                payload.insert(
                    "body_closure".to_string(),
                    serde_json::json!({
                        "var_name": var_name,
                        "body_child_idx": body_child_idx,
                    }),
                );
                ctx.registry
                    .construct("type.pi", &Value::Object(payload))
                    .ok_or_else(|| {
                        LangError::engine("typecheck: construct_pi: failed to mint type.pi value")
                    })
            }
            "construct_sigma" => {
                // Construct a dependent pair type `Σ (x: param) × body(x)`.
                // Same structure as `construct_pi` but mints `type.sigma`.
                let var_name = lit.and_then(|v| v.as_str()).unwrap_or("x").to_string();
                let param = if args.is_empty() {
                    incoming.clone().unwrap_or(Value::Null)
                } else {
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?
                };
                let body_child = child_ref(ast, node_ref, 1)?;
                let body_child_idx = body_child.index();
                let mut payload = serde_json::Map::new();
                payload.insert("param".to_string(), param);
                payload.insert(
                    "body_closure".to_string(),
                    serde_json::json!({
                        "var_name": var_name,
                        "body_child_idx": body_child_idx,
                    }),
                );
                ctx.registry
                    .construct("type.sigma", &Value::Object(payload))
                    .ok_or_else(|| {
                        LangError::engine(
                            "typecheck: construct_sigma: failed to mint type.sigma value",
                        )
                    })
            }
            "pi_apply" => {
                // Apply a Pi (or Sigma) closure to an argument type value,
                // returning the body type. `args[0]` is a sub-scheme producing
                // the Pi type value; `args[1]` (or `incoming`) is a sub-scheme
                // producing the argument's type value. The engine marks Γ,
                // extends it with the bound variable → argument type, synths
                // the body child, and restores Γ (ADR-0054 closure application).
                if args.is_empty() {
                    return Err(LangError::engine(
                        "typecheck: pi_apply requires at least 1 arg (Pi type sub-scheme)",
                    ));
                }
                let pi_val =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let arg_type = if args.len() > 1 {
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let body_type = self.apply_type_closure(lang, ast, &pi_val, &arg_type, ctx)?;
                Ok(body_type)
            }
            "unify_pi" => {
                // Unify two Pi (or Sigma) types with closure-based bodies.
                // `args[0]` and `args[1]` are sub-schemes producing the two
                // type values. Unifies the `param` fields, then creates a
                // fresh metavariable, applies both closures with it, and
                // unifies the resulting body types (ADR-0054 closure
                // unification — mirrors legacy `unify` for `Value::Pi`).
                if args.len() < 2 {
                    return Err(LangError::engine(
                        "typecheck: unify_pi requires 2 args (two Pi type sub-schemes)",
                    ));
                }
                let pi_a =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let pi_b = self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                // Unify param fields.
                let param_a = pi_a.get("param").unwrap_or(&Value::Null);
                let param_b = pi_b.get("param").unwrap_or(&Value::Null);
                self.unify(param_a, param_b, ctx)?;
                // Create a fresh metavariable as the stand-in argument type.
                let fresh_id = ctx.fresh_meta();
                let fresh = meta_value(fresh_id);
                // Apply both closures with the fresh metavariable.
                let body_a = self.apply_type_closure(lang, ast, &pi_a, &fresh, ctx)?;
                let body_b = self.apply_type_closure(lang, ast, &pi_b, &fresh, ctx)?;
                // Unify the resulting body types.
                self.unify(&body_a, &body_b, ctx)?;
                Ok(Value::Null)
            }
            "check_pi_body" => {
                // Check a lambda/function body against a Pi type's body
                // closure. `args[0]` is a sub-scheme producing the Pi type
                // value; the current node's child 1 is the body to check.
                // The engine marks Γ, extends it with the bound variable →
                // Pi's param type, applies the closure to get the expected
                // body type, checks child 1 against it, and restores Γ
                // (ADR-0054 lambda checking against Pi types).
                if args.is_empty() {
                    return Err(LangError::engine(
                        "typecheck: check_pi_body requires 1 arg (Pi type sub-scheme)",
                    ));
                }
                let pi_val =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let param_type = pi_val.get("param").cloned().unwrap_or(Value::Null);
                // Extract closure info to get the variable name.
                let body_closure = pi_val.get("body_closure").ok_or_else(|| {
                    LangError::engine("typecheck: check_pi_body: Pi type has no body_closure")
                })?;
                let var_name = body_closure
                    .get("var_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("x");
                // Mark Γ, extend with the variable → param type.
                let mark = ctx.mark();
                ctx.extend(var_name.to_string(), param_type.clone());
                // Apply the closure with the param type to get the expected
                // body type.
                let expected_body =
                    self.apply_type_closure(lang, ast, &pi_val, &param_type, ctx)?;
                // Check child 1 (the lambda body) against the expected body type.
                let body_child = child_ref(ast, node_ref, 1)?;
                self.check_node(lang, ast, body_child, &expected_body, ctx)?;
                // Restore Γ.
                ctx.restore(mark);
                Ok(Value::Null)
            }
            // --- ADR-0054 Forall (parametric polymorphism) ---
            //
            // Forall is like Pi but for type-level abstraction:
            // `∀ (x:Kind) → body(x)`. The parameter is a *type*, not a value.
            // The forall type value carries a `kind` field (the kind of the
            // type parameter) and a `body_closure` marker, mirroring Pi.
            "construct_forall" => {
                // Construct a polymorphic type `∀ (x: kind) → body(x)`.
                // `lit` is the variable name (default "x"); `args[0]` is a
                // sub-scheme producing the kind of the type parameter. The
                // body child is child 1 of the current node; its arena index
                // is stored in the closure marker for lazy evaluation via
                // `forall_apply`.
                let var_name = lit.and_then(|v| v.as_str()).unwrap_or("x").to_string();
                let kind = if args.is_empty() {
                    incoming.clone().unwrap_or(Value::Null)
                } else {
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?
                };
                let body_child = child_ref(ast, node_ref, 1)?;
                let body_child_idx = body_child.index();
                let mut payload = serde_json::Map::new();
                payload.insert("kind".to_string(), kind);
                payload.insert(
                    "body_closure".to_string(),
                    serde_json::json!({
                        "var_name": var_name,
                        "body_child_idx": body_child_idx,
                    }),
                );
                ctx.registry
                    .construct("type.forall", &Value::Object(payload))
                    .ok_or_else(|| {
                        LangError::engine(
                            "typecheck: construct_forall: failed to mint type.forall value",
                        )
                    })
            }
            "forall_apply" => {
                // Instantiate a polymorphic Forall type with a type argument,
                // returning the body type. `args[0]` is a sub-scheme producing
                // the Forall type value; `args[1]` (or `incoming`) is a
                // sub-scheme producing the type argument. The engine marks Γ,
                // extends it with the bound variable → type argument, synths
                // the body child, and restores Γ (ADR-0054 closure
                // application — mirrors `pi_apply`).
                if args.is_empty() {
                    return Err(LangError::engine(
                        "typecheck: forall_apply requires at least 1 arg (Forall type sub-scheme)",
                    ));
                }
                let forall_val =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let arg_type = if args.len() > 1 {
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let body_type = self.apply_type_closure(lang, ast, &forall_val, &arg_type, ctx)?;
                Ok(body_type)
            }
            "unify_forall" => {
                // Unify two Forall types with closure-based bodies.
                // `args[0]` and `args[1]` are sub-schemes producing the two
                // type values. Unifies the `kind` fields, then creates a
                // fresh metavariable, applies both closures with it, and
                // unifies the resulting body types (ADR-0054 closure
                // unification — mirrors `unify_pi`).
                if args.len() < 2 {
                    return Err(LangError::engine(
                        "typecheck: unify_forall requires 2 args (two Forall type sub-schemes)",
                    ));
                }
                let fa_a =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let fa_b = self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                // Unify kind fields.
                let kind_a = fa_a.get("kind").unwrap_or(&Value::Null);
                let kind_b = fa_b.get("kind").unwrap_or(&Value::Null);
                self.unify(kind_a, kind_b, ctx)?;
                // Create a fresh metavariable as the stand-in type argument.
                let fresh_id = ctx.fresh_meta();
                let fresh = meta_value(fresh_id);
                // Apply both closures with the fresh metavariable.
                let body_a = self.apply_type_closure(lang, ast, &fa_a, &fresh, ctx)?;
                let body_b = self.apply_type_closure(lang, ast, &fa_b, &fresh, ctx)?;
                // Unify the resulting body types.
                self.unify(&body_a, &body_b, ctx)?;
                Ok(Value::Null)
            }
            // --- ADR-0054 Vau (fexpr) type support ---
            //
            // Vau is like Lambda but the parameter is NOT evaluated before
            // substitution. The type is `type.vau` — like `type.pi` but
            // marked as unevaluated. The vau type value carries a `param`
            // field and a `body_closure` marker, mirroring Pi.
            "construct_vau_type" => {
                // Construct a vau (fexpr) type `Vau (x: param) → body(x)`.
                // Same structure as `construct_pi` but mints `type.vau`.
                let var_name = lit.and_then(|v| v.as_str()).unwrap_or("x").to_string();
                let param = if args.is_empty() {
                    incoming.clone().unwrap_or(Value::Null)
                } else {
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?
                };
                let body_child = child_ref(ast, node_ref, 1)?;
                let body_child_idx = body_child.index();
                let mut payload = serde_json::Map::new();
                payload.insert("param".to_string(), param);
                payload.insert(
                    "body_closure".to_string(),
                    serde_json::json!({
                        "var_name": var_name,
                        "body_child_idx": body_child_idx,
                    }),
                );
                ctx.registry
                    .construct("type.vau", &Value::Object(payload))
                    .ok_or_else(|| {
                        LangError::engine(
                            "typecheck: construct_vau_type: failed to mint type.vau value",
                        )
                    })
            }
            "vau_apply" => {
                // Apply a Vau type closure to an argument type value,
                // returning the body type. `args[0]` is a sub-scheme producing
                // the Vau type value; `args[1]` (or `incoming`) is a
                // sub-scheme producing the argument's type value. The engine
                // marks Γ, extends it with the bound variable → argument
                // type, synths the body child, and restores Γ (ADR-0054
                // closure application — mirrors `pi_apply`).
                if args.is_empty() {
                    return Err(LangError::engine(
                        "typecheck: vau_apply requires at least 1 arg (Vau type sub-scheme)",
                    ));
                }
                let vau_val =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let arg_type = if args.len() > 1 {
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let body_type = self.apply_type_closure(lang, ast, &vau_val, &arg_type, ctx)?;
                Ok(body_type)
            }
            "unify_vau" => {
                // Unify two Vau types with closure-based bodies.
                // `args[0]` and `args[1]` are sub-schemes producing the two
                // type values. Unifies the `param` fields, then creates a
                // fresh metavariable, applies both closures with it, and
                // unifies the resulting body types (ADR-0054 closure
                // unification — mirrors `unify_pi`).
                if args.len() < 2 {
                    return Err(LangError::engine(
                        "typecheck: unify_vau requires 2 args (two Vau type sub-schemes)",
                    ));
                }
                let vau_a =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let vau_b = self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?;
                // Unify param fields.
                let param_a = vau_a.get("param").unwrap_or(&Value::Null);
                let param_b = vau_b.get("param").unwrap_or(&Value::Null);
                self.unify(param_a, param_b, ctx)?;
                // Create a fresh metavariable as the stand-in argument type.
                let fresh_id = ctx.fresh_meta();
                let fresh = meta_value(fresh_id);
                // Apply both closures with the fresh metavariable.
                let body_a = self.apply_type_closure(lang, ast, &vau_a, &fresh, ctx)?;
                let body_b = self.apply_type_closure(lang, ast, &vau_b, &fresh, ctx)?;
                // Unify the resulting body types.
                self.unify(&body_a, &body_b, ctx)?;
                Ok(Value::Null)
            }
            // --- ADR-0054 equality type support (Eq, Refl, Empty) ---
            //
            // These ops bring propositional equality into the data-driven
            // engine. `Eq(a, b)` is the equality type — inhabited (by `Refl`)
            // iff `a == b`. `Refl` is the proof of equality. `Empty` is the
            // empty type (contradiction), used when equality fails
            // (ADR-0054 legacy migration — mirrors legacy `Value::Eq`,
            // `Value::Refl`, `Value::Empty`).
            "construct_eq" => {
                // Construct a `type.eq` tagged value with `left` and `right`
                // fields. `args[0]` is the left side sub-scheme; `args[1]` is
                // the right side sub-scheme (ADR-0054).
                if args.len() < 2 {
                    return Err(LangError::engine(
                        "construct_eq: requires 2 args (left, right)",
                    ));
                }
                let left =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let right =
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming.clone())?;
                let payload = serde_json::json!({"left": left, "right": right});
                ctx.registry
                    .construct("type.eq", &payload)
                    .ok_or_else(|| LangError::engine("construct_eq: construct failed"))
            }
            "construct_empty" => {
                // Construct a `type.empty` tagged value (the empty type /
                // contradiction). It carries no fields (ADR-0054).
                ctx.registry
                    .construct("type.empty", &Value::Null)
                    .ok_or_else(|| LangError::engine("construct_empty: construct failed"))
            }
            "check_refl" => {
                // Check a `refl` node against an expected `type.eq` type. The
                // check succeeds iff the two sides are equal. The expected
                // type comes from `incoming` (threaded by the `check` scheme)
                // or `args[0]` (ADR-0054).
                let expected = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                // Check that expected is a type.eq.
                let tag = ctx.registry.as_type_tag(&expected).ok_or_else(|| {
                    LangError::engine("check_refl: expected type is not a tagged type value")
                })?;
                if tag != "type.eq" {
                    return Err(LangError::engine(format!(
                        "check_refl: expected type {} is not type.eq",
                        tag
                    )));
                }
                // Extract left and right sides.
                let left = expected.get("left").unwrap_or(&Value::Null);
                let right = expected.get("right").unwrap_or(&Value::Null);
                // Check that left == right (they must be equal for refl to be
                // a valid proof).
                if left == right {
                    Ok(Value::Null)
                } else {
                    Err(LangError::engine(format!(
                        "check_refl: cannot prove {:?} == {:?}",
                        left, right
                    )))
                }
            }
            "eq_reduce" => {
                // Reduce an `Eq(a, b)` type: if `a == b`, returns `type.refl`
                // (the inhabited case); otherwise returns `type.empty`
                // (ADR-0054). The Eq type value comes from `args[0]` or
                // `incoming`.
                let eq_val = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let left = eq_val.get("left").unwrap_or(&Value::Null);
                let right = eq_val.get("right").unwrap_or(&Value::Null);
                if left == right {
                    // Equality holds — return type.refl (the inhabited case).
                    ctx.registry
                        .construct("type.refl", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine("eq_reduce: construct failed for type.refl")
                        })
                } else {
                    // Equality fails — return type.empty.
                    ctx.registry
                        .construct("type.empty", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine("eq_reduce: construct failed for type.empty")
                        })
                }
            }
            // --- ADR-0054 universe level support ---
            //
            // Universe types `Type_n` are types whose elements are types of
            // level < n. The cumulative hierarchy is `Type_0 ⊆ Type_1 ⊆ ...`
            // (mirrors the legacy `plugin-typecheck-nbe` `Level` type). These
            // ops let type rules construct universes, extract their levels,
            // and compute `max` levels for Pi/Sigma result universes.
            "construct_universe" => {
                // Construct a `type.universe` value carrying a `level` field.
                // `lit` is the level (u64), or `args[0]` is a sub-scheme
                // evaluating to the level (u64). Defaults to 0 when neither is
                // present (base universe `Type_0`).
                let level = if let Some(lit_val) = lit {
                    lit_val.as_u64().ok_or_else(|| {
                        LangError::engine("construct_universe: lit must be a u64 level")
                    })? as u32
                } else if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                        .as_u64()
                        .ok_or_else(|| {
                            LangError::engine(
                                "construct_universe: arg must evaluate to a u64 level",
                            )
                        })? as u32
                } else {
                    0
                };
                let mut payload = serde_json::Map::new();
                payload.insert("level".to_string(), Value::from(level));
                ctx.registry
                    .construct("type.universe", &Value::Object(payload))
                    .ok_or_else(|| {
                        LangError::engine("construct_universe: construct failed for type.universe")
                    })
            }
            "universe_level" => {
                // Extract the universe level of a type value. For a
                // `type.universe` value, returns its `level` field. For other
                // type values (base types like `type.number`), returns 0
                // (base types live in `Type_0`). The type value comes from
                // `args[0]` (a sub-scheme) or `incoming`.
                let type_val = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                if let Some(tag) = ctx.registry.as_type_tag(&type_val) {
                    if tag == "type.universe" {
                        let level = type_val.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
                        return Ok(Value::from(level));
                    }
                }
                // For other type values, return 0 (base types live in Type_0).
                Ok(Value::from(0u64))
            }
            "universe_max" => {
                // Compute the max of two universe levels (u64). `args[0]` and
                // `args[1]` are sub-schemes evaluating to levels. Used by
                // Pi/Sigma rules to compute the result universe as
                // `max(param_universe, body_universe)`.
                if args.len() < 2 {
                    return Err(LangError::engine(
                        "universe_max requires 2 args (two level sub-schemes)",
                    ));
                }
                let level_a = self
                    .eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?
                    .as_u64()
                    .ok_or_else(|| LangError::engine("universe_max: arg 0 is not a u64"))?;
                let level_b = self
                    .eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming)?
                    .as_u64()
                    .ok_or_else(|| LangError::engine("universe_max: arg 1 is not a u64"))?;
                Ok(Value::from(level_a.max(level_b)))
            }
            // --- ADR-0054 Membership types (Is, Collection, Membership) ---
            //
            // Membership types model the proposition `value ∈ collection`.
            // `Collection(name)` is a named collection (e.g. `type.number`,
            // `type.string`). `Membership(value, collection)` is the type of
            // `value ∈ collection`. `IsEvidence` is the proof when membership
            // holds; `Empty` is the result when it doesn't. The core
            // operation is `primitive_member(value, collection)` — it checks
            // if a JSON value is a member of a primitive collection
            // (type.number ↔ JSON number, etc.), mirroring the legacy
            // `plugin-typecheck-nbe` `primitive_member` function.
            "construct_membership" => {
                // Create a `type.membership` tagged value with `value` and
                // `collection` fields. `args[0]` is the value sub-scheme;
                // `args[1]` is the collection type sub-scheme.
                if args.len() < 2 {
                    return Err(LangError::engine(
                        "construct_membership: requires 2 args (value, collection)",
                    ));
                }
                let value =
                    self.eval_type_scheme(lang, ast, node_ref, &args[0], ctx, incoming.clone())?;
                let collection =
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming.clone())?;
                let payload = serde_json::json!({"value": value, "collection": collection});
                ctx.registry
                    .construct("type.membership", &payload)
                    .ok_or_else(|| LangError::engine("construct_membership: construct failed"))
            }
            "check_membership" => {
                // Check if a JSON value is a member of a collection type.
                // This is the core membership oracle. `args[0]` is the JSON
                // value sub-scheme (or `incoming`); `args[1]` is the
                // collection type sub-scheme (or `lit` is the collection tag
                // string). Returns `type.is_evidence` if membership holds,
                // `type.empty` otherwise.
                let value = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let collection = if args.len() > 1 {
                    self.eval_type_scheme(lang, ast, node_ref, &args[1], ctx, incoming.clone())?
                } else if let Some(lit_val) = lit {
                    // lit is the collection tag string.
                    ctx.registry
                        .construct(
                            lit_val.as_str().ok_or_else(|| {
                                LangError::engine(
                                    "check_membership: lit must be a collection tag string",
                                )
                            })?,
                            &Value::Null,
                        )
                        .ok_or_else(|| LangError::engine("check_membership: construct failed"))?
                } else {
                    return Err(LangError::engine(
                        "check_membership: no collection specified",
                    ));
                };
                // Get the collection tag.
                let collection_tag = ctx.registry.as_type_tag(&collection).ok_or_else(|| {
                    LangError::engine("check_membership: collection is not a tagged type value")
                })?;
                // Check membership (mirrors legacy `primitive_member`).
                let is_member = match collection_tag {
                    "type.number" => value.is_number(),
                    "type.string" => value.is_string(),
                    "type.bool" => value.is_boolean(),
                    "type.null" => value.is_null(),
                    "type.array" => value.is_array(),
                    "type.object" => value.is_object(),
                    _ => false,
                };
                if is_member {
                    // Return IsEvidence (proof that membership holds).
                    ctx.registry
                        .construct("type.is_evidence", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine(
                                "check_membership: construct failed for type.is_evidence",
                            )
                        })
                } else {
                    // Return Empty (membership doesn't hold).
                    ctx.registry
                        .construct("type.empty", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine("check_membership: construct failed for type.empty")
                        })
                }
            }
            "eval_membership" => {
                // Reduce a `type.membership` value to either
                // `type.is_evidence` (if membership holds) or `type.empty`
                // (if not). `args[0]` or `incoming` is the membership type
                // value.
                let membership_val = if let Some(arg) = args.first() {
                    self.eval_type_scheme(lang, ast, node_ref, arg, ctx, incoming.clone())?
                } else {
                    incoming.clone().unwrap_or(Value::Null)
                };
                let value = membership_val.get("value").unwrap_or(&Value::Null);
                let collection = membership_val.get("collection").unwrap_or(&Value::Null);
                let collection_tag = ctx.registry.as_type_tag(collection).ok_or_else(|| {
                    LangError::engine("eval_membership: collection is not a tagged type value")
                })?;
                let is_member = match collection_tag {
                    "type.number" => value.is_number(),
                    "type.string" => value.is_string(),
                    "type.bool" => value.is_boolean(),
                    "type.null" => value.is_null(),
                    "type.array" => value.is_array(),
                    "type.object" => value.is_object(),
                    _ => false,
                };
                if is_member {
                    ctx.registry
                        .construct("type.is_evidence", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine(
                                "eval_membership: construct failed for type.is_evidence",
                            )
                        })
                } else {
                    ctx.registry
                        .construct("type.empty", &Value::Null)
                        .ok_or_else(|| {
                            LangError::engine("eval_membership: construct failed for type.empty")
                        })
                }
            }
            "construct_collection" => {
                // Create a `type.collection` tagged value with a `name`
                // field. `lit` is the collection name string.
                let name = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine("construct_collection: lit must be a collection name string")
                })?;
                let payload = serde_json::json!({"name": name});
                ctx.registry
                    .construct("type.collection", &payload)
                    .ok_or_else(|| LangError::engine("construct_collection: construct failed"))
            }
            "marshalable" => {
                // Check that the incoming type value is an FFI-marshalable
                // type (ADR-0083).
                let ty = incoming.as_ref().ok_or_else(|| {
                    LangError::engine("typecheck: marshalable requires an incoming type value")
                })?;
                if is_marshalable(&ctx.registry, ty) {
                    Ok(Value::Bool(true))
                } else {
                    let tag = ctx.registry.as_type_tag(ty).unwrap_or("unknown");
                    Err(LangError::engine(format!(
                        "typecheck: type '{}' is not FFI-marshalable (ADR-0083)",
                        tag
                    )))
                }
            }
            "foreign_type" => {
                // Look up a Foreign op's type signature from the module
                // registry (ADR-0083). The op name comes from the lit
                // payload (a string).
                let op_name = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine(
                        "typecheck: foreign_type requires a string lit (op name)",
                    )
                })?;
                let modules = self.modules.as_ref().ok_or_else(|| {
                    LangError::engine(
                        "typecheck: foreign_type requires a module registry (call with_modules)",
                    )
                })?;
                modules.lookup_type_signature(op_name).ok_or_else(|| {
                    LangError::engine(format!(
                        "typecheck: foreign op '{}' has no registered type signature",
                        op_name
                    ))
                })
            }
            other => Err(LangError::engine(format!(
                "typecheck: unknown judgment op '{}'",
                other
            ))),
        }
    }

    /// Apply a type closure (from a `type.pi`, `type.sigma`, `type.forall`,
    /// or `type.vau` value) to an argument type value, returning the body
    /// type. The closure marker (`body_closure`) records the variable name
    /// and the AST child index of the body. The engine marks Γ, extends it
    /// with the variable → argument type, synths the body child, and
    /// restores Γ (ADR-0054).
    fn apply_type_closure(
        &self,
        lang: &Language,
        ast: &Ast,
        type_val: &Value,
        arg_type: &Value,
        ctx: &mut TypecheckContext,
    ) -> Result<Value, LangError> {
        let body_closure = type_val.get("body_closure").ok_or_else(|| {
            LangError::engine("typecheck: apply_type_closure: type value has no body_closure field")
        })?;
        let var_name = body_closure
            .get("var_name")
            .and_then(|v| v.as_str())
            .unwrap_or("x");
        let body_child_idx = body_closure
            .get("body_child_idx")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                LangError::engine(
                    "typecheck: apply_type_closure: body_closure missing body_child_idx",
                )
            })? as usize;
        // Mark Γ, extend with the variable → argument type, synth the body
        // child, restore Γ (scoped closure application).
        let mark = ctx.mark();
        ctx.extend(var_name.to_string(), arg_type.clone());
        let body_type = self.synth_node(lang, ast, AstRef::new(body_child_idx), ctx)?;
        ctx.restore(mark);
        Ok(body_type)
    }

    /// Occurs-check: returns `true` if the metavariable `meta_id` occurs
    /// anywhere in `v` (recursively, through arrays and objects). This
    /// prevents solving a metavariable to a value that contains itself,
    /// which would create an infinite loop (ADR-0054 legacy migration).
    fn occurs_in(&self, meta_id: usize, v: &Value) -> bool {
        if let Some(obj) = v.as_object() {
            if let Some(meta) = obj.get("__meta__") {
                if meta.as_u64() == Some(meta_id as u64) {
                    return true;
                }
            }
            // Recurse into all object values.
            for (_, val) in obj {
                if self.occurs_in(meta_id, val) {
                    return true;
                }
            }
        }
        if let Some(arr) = v.as_array() {
            for val in arr {
                if self.occurs_in(meta_id, val) {
                    return true;
                }
            }
        }
        false
    }

    /// Resolve (zonk) all solved metavariables inside a type value.
    ///
    /// Follows `{"__meta__": id}` chains: if a metavariable has been solved
    /// in `ctx.metas`, replaces it with its solution (recursively). Unsolved
    /// metavariables are left as-is. Recurses into object fields and array
    /// elements so that nested metas (e.g. inside `type.array{element: ?}`)
    /// are resolved too. This is the read-back step that turns the NBE
    /// metavariable store into a concrete type value (ADR-0054).
    ///
    /// Cycle-safe: tracks the meta ids on the current DFS path and breaks
    /// any cycle (e.g. from mutually-solved metas `m0 := m1, m1 := m0`) by
    /// leaving the revisited meta as-is, preventing infinite recursion.
    fn zonk(&self, v: &Value, ctx: &TypecheckContext) -> Value {
        let mut visited = std::collections::HashSet::new();
        self.zonk_inner(v, ctx, &mut visited)
    }

    fn zonk_inner(
        &self,
        v: &Value,
        ctx: &TypecheckContext,
        visited: &mut std::collections::HashSet<u64>,
    ) -> Value {
        if let Some(id) = extract_meta_id(v) {
            if visited.contains(&id) {
                // Cycle — break it by leaving the meta as-is.
                return v.clone();
            }
            if let Some(Some(solved)) = ctx.metas.get(id as usize) {
                visited.insert(id);
                let out = self.zonk_inner(solved, ctx, visited);
                visited.remove(&id);
                return out;
            }
            return v.clone();
        }
        match v {
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, val) in map {
                    out.insert(k.clone(), self.zonk_inner(val, ctx, visited));
                }
                Value::Object(out)
            }
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|val| self.zonk_inner(val, ctx, visited))
                    .collect(),
            ),
            _ => v.clone(),
        }
    }

    /// Extract the `(name, value_ref)` binding from an `assign` node.
    ///
    /// Supports two payload shapes (mirroring [`Self::synth_let`]'s
    /// `synth_let` judgment):
    /// - v0.1 Pratt-parsed: `payload.name` holds the binding name and child 0
    ///   is the value expression.
    /// - Legacy: children are `[var(name), value]` where the `var` node's
    ///   payload is the name string.
    fn extract_assign_binding(
        &self,
        ast: &Ast,
        assign_node: &handl_language::AstNode,
    ) -> Result<(String, AstRef), LangError> {
        let assign_children = assign_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine("typecheck: synth_rec: assign node has no children array")
            })?;
        // v0.1: name in payload.name, value is child 0.
        if let Some(name) = assign_node.payload.get("name").and_then(|v| v.as_str()) {
            if assign_children.is_empty() {
                return Err(LangError::engine(
                    "typecheck: synth_rec: assign node with payload.name needs a value child",
                ));
            }
            let value_idx = assign_children[0].as_u64().ok_or_else(|| {
                LangError::engine("typecheck: synth_rec: assign value index is not a u64")
            })? as usize;
            return Ok((name.to_string(), AstRef::new(value_idx)));
        }
        // Legacy: children [var(name), value].
        if assign_children.len() < 2 {
            return Err(LangError::engine(
                "typecheck: synth_rec: assign node needs at least 2 children (var, value)",
            ));
        }
        let var_idx = assign_children[0].as_u64().ok_or_else(|| {
            LangError::engine("typecheck: synth_rec: assign var index is not a u64")
        })? as usize;
        let value_idx = assign_children[1].as_u64().ok_or_else(|| {
            LangError::engine("typecheck: synth_rec: assign value index is not a u64")
        })? as usize;
        let var_node = ast.get(AstRef::new(var_idx)).ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: synth_rec: var node {} out of bounds",
                var_idx
            ))
        })?;
        let name = var_node.payload.as_str().ok_or_else(|| {
            LangError::engine_node(
                "typecheck: synth_rec: var node payload is not a string",
                var_node,
            )
        })?;
        Ok((name.to_string(), AstRef::new(value_idx)))
    }

    /// Typecheck a `rec` region by computing the fixpoint of its binding
    /// types (ADR-0074 ID.3 — Paterson loop).
    ///
    /// The `rec` node is a flattened block (grammar `grammar.rec.prefix` with
    /// `flatten: true`): its `children` are individual expressions. `assign`
    /// children define bindings (`name = value`); other children are
    /// expressions evaluated for effect/result. Bindings may reference each
    /// other cyclically — the region is a cyclic subgraph.
    ///
    /// The typing algorithm:
    /// 1. Collect the `assign` bindings and note any `return` child and the
    ///    last child.
    /// 2. Allocate a fresh metavariable per binding and extend Γ with
    ///    `(name, meta)`. This lets a binding's value synth resolve sibling
    ///    references to their (initially unknown) types.
    /// 3. Iterate to fixpoint: synth each binding's value type under the
    ///    current Γ and unify it with the binding's metavariable (solving or
    ///    refining it). Repeat until the zonked binding types stabilize.
    ///    A fuel limit (100 iterations) prevents divergence; exhausting it
    ///    is a compile error ("cannot determine fixpoint type for rec
    ///    region").
    /// 4. The result type is the zonked type of the `return` child's value
    ///    (if present), otherwise the zonked type of the last child.
    ///
    /// Termination is NOT checked at the type level (ADR-0074 ID.3) — it is a
    /// runtime/effect concern. The occurs-check in [`Self::unify`] rejects
    /// genuinely cyclic (infinite) types such as `rec { a = [a]; a }`.
    fn synth_rec_judgment(
        &self,
        lang: &Language,
        ast: &Ast,
        node_ref: AstRef,
        ctx: &mut TypecheckContext,
    ) -> Result<Value, LangError> {
        const REC_FIXPOINT_FUEL: usize = 100;
        let node = ast.get(node_ref).ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: synth_rec: node {} out of bounds",
                node_ref.index()
            ))
        })?;
        let children = node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine_node("typecheck: synth_rec: node has no children array", node)
            })?;
        // Empty rec region → type.unit (mirrors empty block).
        if children.is_empty() {
            return ctx
                .registry
                .construct("type.unit", &Value::Null)
                .ok_or_else(|| {
                    LangError::engine_node(
                        "typecheck: synth_rec: construct failed for type.unit",
                        node,
                    )
                });
        }
        // Collect bindings (assign nodes), the return child, and the last
        // child. Each binding records (name, value_ref).
        let mut bindings: Vec<(String, AstRef)> = Vec::new();
        let mut return_ref: Option<AstRef> = None;
        let mut last_ref: Option<AstRef> = None;
        for child_entry in children {
            let idx = child_entry.as_u64().ok_or_else(|| {
                LangError::engine_node("typecheck: synth_rec: child index is not a u64", node)
            })? as usize;
            let child = AstRef::new(idx);
            let child_node = ast.get(child).ok_or_else(|| {
                LangError::engine(format!(
                    "typecheck: synth_rec: child node {} out of bounds",
                    idx
                ))
            })?;
            if is_assign_node(lang, &child_node) {
                let (name, value_ref) = self.extract_assign_binding(ast, child_node)?;
                bindings.push((name, value_ref));
            } else if is_return_node(lang, &child_node) {
                return_ref = Some(child);
            }
            last_ref = Some(child);
        }
        // Scope Γ for the rec region: all binding extensions are popped on
        // exit so the region's locals don't leak (ADR-0054 scoped context).
        let mark = ctx.mark();
        // Allocate a fresh metavariable per binding and extend Γ.
        let mut binding_metas: Vec<u64> = Vec::with_capacity(bindings.len());
        for (name, _value_ref) in &bindings {
            let mid = ctx.fresh_meta();
            binding_metas.push(mid);
            ctx.extend(name.clone(), meta_value(mid));
        }
        // Fixpoint iteration: synth each binding's value and unify with its
        // meta until the zonked binding types stabilize.
        let mut converged = false;
        for _ in 0..REC_FIXPOINT_FUEL {
            let prev: Vec<Value> = binding_metas
                .iter()
                .map(|&id| self.zonk(&meta_value(id), ctx))
                .collect();
            for ((_name, value_ref), &mid) in bindings.iter().zip(binding_metas.iter()) {
                let derived = self.synth_node(lang, ast, *value_ref, ctx)?;
                // Zonk the derived type so sibling references that already
                // resolved to a solved meta are reconciled before unifying
                // (this prevents forming meta cycles such as m0 := m1
                // followed by m1 := m0). Then unify the binding's
                // (possibly already-solved) meta with the derived type.
                let derived = self.zonk(&derived, ctx);
                let meta_val = self.zonk(&meta_value(mid), ctx);
                self.unify(&meta_val, &derived, ctx)?;
            }
            let curr: Vec<Value> = binding_metas
                .iter()
                .map(|&id| self.zonk(&meta_value(id), ctx))
                .collect();
            if prev == curr {
                converged = true;
                break;
            }
        }
        if !converged {
            ctx.restore(mark);
            return Err(LangError::engine(
                "typecheck: cannot determine fixpoint type for rec region",
            ));
        }
        // Determine the result type. A `return` child wins; otherwise the
        // last child's type. If the last child is itself an `assign`, the
        // rec result is that binding's value type (not type.unit), mirroring
        // the eval semantics where the last binding's value is the result.
        let result_type = if let Some(ret_ref) = return_ref {
            let value_ref = child_ref(ast, ret_ref, 0)?;
            let t = self.synth_node(lang, ast, value_ref, ctx)?;
            self.zonk(&t, ctx)
        } else if let Some(lref) = last_ref {
            let lnode = ast.get(lref).ok_or_else(|| {
                LangError::engine(format!(
                    "typecheck: synth_rec: last node {} out of bounds",
                    lref.index()
                ))
            })?;
            if is_assign_node(lang, &lnode) {
                let (_name, value_ref) = self.extract_assign_binding(ast, lnode)?;
                let t = self.synth_node(lang, ast, value_ref, ctx)?;
                self.zonk(&t, ctx)
            } else {
                let t = self.synth_node(lang, ast, lref, ctx)?;
                self.zonk(&t, ctx)
            }
        } else {
            // No children (handled above) — defensive fallback.
            ctx.registry
                .construct("type.unit", &Value::Null)
                .ok_or_else(|| {
                    LangError::engine_node(
                        "typecheck: synth_rec: construct failed for type.unit",
                        node,
                    )
                })?
        };
        ctx.restore(mark);
        Ok(result_type)
    }

    /// Row-polymorphic record unification (ADR-0074 ID.7).
    ///
    /// Unifies two `type.record` values, handling open records (with a `rest`
    /// row variable) and closed records (without `rest`). The algorithm:
    ///
    /// - **Both closed**: unify common fields; require identical key sets
    ///   (extra fields on either side are an error).
    /// - **One open, one closed**: unify common fields; the closed record's
    ///   extra fields (not in the open record's known fields) are assigned to
    ///   the open record's row variable. If the open record has known fields
    ///   absent from the closed record, that is an error (the closed record
    ///   cannot supply them).
    /// - **Both open**: unify common fields; create a fresh row variable `r3`,
    ///   then assign `r1 = {extra_b | r3}` and `r2 = {extra_a | r3}` where
    ///   `extra_a`/`extra_b` are the non-common fields from each side. This is
    ///   the "row unification" algorithm from Daan Leijen's "Extensible
    ///   records with scoped labels".
    ///
    /// Row variables are metavariables (`{"__meta__": id}`); assigning a row
    /// variable solves the metavariable to a `type.record{fields: {...}}`
    /// value (the extra fields, possibly with its own `rest`).
    fn unify_record(
        &self,
        a: &Value,
        b: &Value,
        ctx: &mut TypecheckContext,
    ) -> Result<(), LangError> {
        let fields_a = a.get("fields").and_then(|f| f.as_object()).ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: unify_record: missing 'fields' object in {:?}",
                a
            ))
        })?;
        let fields_b = b.get("fields").and_then(|f| f.as_object()).ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: unify_record: missing 'fields' object in {:?}",
                b
            ))
        })?;
        let rest_a = a.get("rest");
        let rest_b = b.get("rest");

        // Unify common fields (present in both records).
        for (key, va) in fields_a {
            if let Some(vb) = fields_b.get(key) {
                self.unify(va, vb, ctx)?;
            }
        }

        // Collect extra fields: in a but not in b, and in b but not in a.
        let extra_a: serde_json::Map<String, Value> = fields_a
            .iter()
            .filter(|(k, _)| !fields_b.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let extra_b: serde_json::Map<String, Value> = fields_b
            .iter()
            .filter(|(k, _)| !fields_a.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        match (rest_a, rest_b) {
            // Both closed: no extra fields allowed on either side.
            (None, None) => {
                if !extra_a.is_empty() || !extra_b.is_empty() {
                    return Err(LangError::engine(format!(
                        "typecheck: cannot unify closed records with different fields: {:?} vs {:?}",
                        extra_a.keys().collect::<Vec<_>>(),
                        extra_b.keys().collect::<Vec<_>>()
                    )));
                }
                Ok(())
            }
            // a is open, b is closed.
            (Some(rest_a_val), None) => {
                // The open record's known fields not in the closed record
                // cannot be satisfied — the closed record has no more fields.
                if !extra_a.is_empty() {
                    return Err(LangError::engine(format!(
                        "typecheck: cannot unify open record with closed record: \
                         open requires fields {:?} absent from closed record",
                        extra_a.keys().collect::<Vec<_>>()
                    )));
                }
                // The closed record's extra fields go to a's row variable.
                let extra_record = self.construct_record(&extra_b, None, ctx)?;
                self.unify(rest_a_val, &extra_record, ctx)
            }
            // a is closed, b is open.
            (None, Some(rest_b_val)) => {
                if !extra_b.is_empty() {
                    return Err(LangError::engine(format!(
                        "typecheck: cannot unify closed record with open record: \
                         open requires fields {:?} absent from closed record",
                        extra_b.keys().collect::<Vec<_>>()
                    )));
                }
                let extra_record = self.construct_record(&extra_a, None, ctx)?;
                self.unify(&extra_record, rest_b_val, ctx)
            }
            // Both open: distribute extra fields to the other's row variable
            // via a shared fresh row variable (Leijen's row unification).
            (Some(rest_a_val), Some(rest_b_val)) => {
                if extra_a.is_empty() && extra_b.is_empty() {
                    // No extra fields — just link the two row variables.
                    return self.unify(rest_a_val, rest_b_val, ctx);
                }
                // Create a fresh row variable shared by both sides.
                let fresh_id = ctx.fresh_meta();
                let fresh_row = meta_value(fresh_id);
                // rest_a := {extra_b | fresh_row}
                let record_b = self.construct_record(&extra_b, Some(&fresh_row), ctx)?;
                self.unify(rest_a_val, &record_b, ctx)?;
                // rest_b := {extra_a | fresh_row}
                let record_a = self.construct_record(&extra_a, Some(&fresh_row), ctx)?;
                self.unify(&record_a, rest_b_val, ctx)?;
                Ok(())
            }
        }
    }

    /// Construct a `type.record` type value from a fields map and an optional
    /// `rest` (row variable). When `rest` is `None`, the record is closed.
    fn construct_record(
        &self,
        fields: &serde_json::Map<String, Value>,
        rest: Option<&Value>,
        ctx: &TypecheckContext,
    ) -> Result<Value, LangError> {
        let mut payload = serde_json::Map::new();
        payload.insert("fields".to_string(), Value::Object(fields.clone()));
        if let Some(rest_val) = rest {
            payload.insert("rest".to_string(), rest_val.clone());
        }
        ctx.registry
            .construct("type.record", &Value::Object(payload))
            .ok_or_else(|| {
                LangError::engine("typecheck: construct_record: failed to mint type.record")
            })
    }

    /// Convert an [`EffectRow`] into a `type.effects` type value
    /// (ADR-0054 / ADR-0076).
    ///
    /// The resulting value is:
    /// ```json
    /// {"__tag__":"type.effects","__registry__":"handl.canonical",
    ///  "ops": ["map", "serialize"], "rest": <null_or_meta_value>}
    /// ```
    /// `ops` is the labelled part of the row (the managed capability names).
    /// `rest` is `null` for a closed row (`tail: None`) or a fresh
    /// metavariable (`{"__meta__": id}`) for an open row (`tail: Some(_)`).
    /// Structural inference via [`infer_effect_row`] always produces closed
    /// rows, so `rest` is `null` in that path; the typechecker may attach an
    /// open tail when ascribing a row-polymorphic type.
    fn effect_row_to_type_value(
        &self,
        row: &EffectRow,
        ctx: &mut TypecheckContext,
    ) -> Result<Value, LangError> {
        // For an open row, allocate a fresh metavariable for the tail. In
        // practice infer_effect_row always produces tail: None, but this
        // handles the general case.
        let rest = match &row.tail {
            None => Value::Null,
            Some(_) => {
                let id = ctx.fresh_meta();
                meta_value(id)
            }
        };
        self.construct_effects(
            &row.ops.iter().cloned().collect::<Vec<_>>(),
            if rest.is_null() { None } else { Some(&rest) },
            ctx,
        )
    }

    /// Construct a `type.effects` type value from an op list and an optional
    /// `rest` (row variable). When `rest` is `None`, the row is closed.
    fn construct_effects(
        &self,
        ops: &[String],
        rest: Option<&Value>,
        ctx: &TypecheckContext,
    ) -> Result<Value, LangError> {
        let ops_arr: Vec<Value> = ops.iter().map(|op| Value::from(op.as_str())).collect();
        let mut payload = serde_json::Map::new();
        payload.insert("ops".to_string(), Value::Array(ops_arr));
        payload.insert("rest".to_string(), rest.cloned().unwrap_or(Value::Null));
        ctx.registry
            .construct("type.effects", &Value::Object(payload))
            .ok_or_else(|| {
                LangError::engine("typecheck: construct_effects: failed to mint type.effects")
            })
    }

    /// Row-polymorphic effect row unification (ADR-0054 / ADR-0076).
    ///
    /// Unifies two `type.effects` values, handling open rows (with a `rest`
    /// row variable) and closed rows (with `rest: null`). The algorithm is
    /// analogous to [`unify_record`]:
    ///
    /// - **Both closed**: require identical op sets (extra ops on either side
    ///   are an error).
    /// - **One open, one closed**: the closed row's extra ops (not in the open
    ///   row's known ops) are assigned to the open row's row variable (solve
    ///   the metavariable to a `type.effects` with those ops). If the open row
    ///   has ops absent from the closed row, that is an error (the closed row
    ///   cannot supply them).
    /// - **Both open**: create a fresh row variable `r3`; assign
    ///   `r1 = {extra_b | r3}` and `r2 = {extra_a | r3}` where `extra_a`/
    ///   `extra_b` are the non-common ops from each side (Leijen's row
    ///   unification).
    ///
    /// Row variables are metavariables (`{"__meta__": id}`); assigning a row
    /// variable solves the metavariable to a `type.effects` value.
    fn unify_effects(
        &self,
        a: &Value,
        b: &Value,
        ctx: &mut TypecheckContext,
    ) -> Result<(), LangError> {
        let ops_a = a.get("ops").and_then(|o| o.as_array()).ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: unify_effects: missing 'ops' array in {:?}",
                a
            ))
        })?;
        let ops_b = b.get("ops").and_then(|o| o.as_array()).ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: unify_effects: missing 'ops' array in {:?}",
                b
            ))
        })?;
        let rest_a = a.get("rest");
        let rest_b = b.get("rest");

        // Collect op names into sorted sets.
        let set_a: std::collections::BTreeSet<String> = ops_a
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        let set_b: std::collections::BTreeSet<String> = ops_b
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        // Extra ops: in a but not in b, and in b but not in a.
        let extra_a: Vec<String> = set_a
            .iter()
            .filter(|op| !set_b.contains(*op))
            .cloned()
            .collect();
        let extra_b: Vec<String> = set_b
            .iter()
            .filter(|op| !set_a.contains(*op))
            .cloned()
            .collect();

        // A row is open iff it has a non-null `rest` field.
        let a_open = rest_a.map(|r| !r.is_null()).unwrap_or(false);
        let b_open = rest_b.map(|r| !r.is_null()).unwrap_or(false);

        match (a_open, b_open) {
            // Both closed: no extra ops allowed on either side.
            (false, false) => {
                if !extra_a.is_empty() || !extra_b.is_empty() {
                    return Err(LangError::engine(format!(
                        "typecheck: cannot unify closed effect rows with different ops: {:?} vs {:?}",
                        extra_a, extra_b
                    )));
                }
                Ok(())
            }
            // a is open, b is closed.
            (true, false) => {
                // The open row's known ops not in the closed row cannot be
                // satisfied — the closed row has no more ops.
                if !extra_a.is_empty() {
                    return Err(LangError::engine(format!(
                        "typecheck: cannot unify open effect row with closed effect row: \
                         open requires ops {:?} absent from closed row",
                        extra_a
                    )));
                }
                // The closed row's extra ops go to a's row variable.
                let extra_effects = self.construct_effects(&extra_b, None, ctx)?;
                let rest_a = rest_a.ok_or_else(|| {
                    LangError::engine("typecheck: unify_effects: rest_a missing for open row")
                })?;
                self.unify(rest_a, &extra_effects, ctx)
            }
            // a is closed, b is open.
            (false, true) => {
                if !extra_b.is_empty() {
                    return Err(LangError::engine(format!(
                        "typecheck: cannot unify closed effect row with open effect row: \
                         open requires ops {:?} absent from closed row",
                        extra_b
                    )));
                }
                let extra_effects = self.construct_effects(&extra_a, None, ctx)?;
                let rest_b = rest_b.ok_or_else(|| {
                    LangError::engine("typecheck: unify_effects: rest_b missing for open row")
                })?;
                self.unify(&extra_effects, rest_b, ctx)
            }
            // Both open: distribute extra ops to the other's row variable via
            // a shared fresh row variable (Leijen's row unification).
            (true, true) => {
                if extra_a.is_empty() && extra_b.is_empty() {
                    // No extra ops — just link the two row variables.
                    let rest_a = rest_a.ok_or_else(|| {
                        LangError::engine("typecheck: unify_effects: rest_a missing for open row")
                    })?;
                    let rest_b = rest_b.ok_or_else(|| {
                        LangError::engine("typecheck: unify_effects: rest_b missing for open row")
                    })?;
                    return self.unify(rest_a, rest_b, ctx);
                }
                // Create a fresh row variable shared by both sides.
                let fresh_id = ctx.fresh_meta();
                let fresh_row = meta_value(fresh_id);
                // rest_a := {extra_b | fresh_row}
                let effects_b = self.construct_effects(&extra_b, Some(&fresh_row), ctx)?;
                let rest_a = rest_a.ok_or_else(|| {
                    LangError::engine("typecheck: unify_effects: rest_a missing for open row")
                })?;
                self.unify(rest_a, &effects_b, ctx)?;
                // rest_b := {extra_a | fresh_row}
                let effects_a = self.construct_effects(&extra_a, Some(&fresh_row), ctx)?;
                let rest_b = rest_b.ok_or_else(|| {
                    LangError::engine("typecheck: unify_effects: rest_b missing for open row")
                })?;
                self.unify(&effects_a, rest_b, ctx)?;
                Ok(())
            }
        }
    }

    /// Check if type `sub` is a subtype of type `sup` (ADR-0054 subtyping).
    ///
    /// This is the subtyping judgment. Unlike [`Self::unify`] (which requires
    /// equality), `check_subtype` supports **universe cumulativity**:
    /// `Type_n ⊆ Type_m` if `n ≤ m` (the cumulative hierarchy
    /// `Type_0 ⊆ Type_1 ⊆ …`). For metavariables, it solves them (same as
    /// `unify`). For other same-tag type values, it falls back to structural
    /// equality via [`Self::unify`] (covariant structural subtyping is a
    /// future extension).
    fn check_subtype(
        &self,
        sub: &Value,
        sup: &Value,
        ctx: &mut TypecheckContext,
    ) -> Result<(), LangError> {
        // If equal, trivially a subtype.
        if sub == sup {
            return Ok(());
        }
        // Metavariable solving (same as unify).
        if let Some(id) = extract_meta_id(sub) {
            if !self.occurs_in(id as usize, sup) {
                ctx.solve_meta(id, sup.clone());
            }
            return Ok(());
        }
        if let Some(id) = extract_meta_id(sup) {
            if !self.occurs_in(id as usize, sub) {
                ctx.solve_meta(id, sub.clone());
            }
            return Ok(());
        }
        // Universe cumulativity: Type_n ⊆ Type_m if n ≤ m.
        if let (Some(tag_sub), Some(tag_sup)) =
            (ctx.registry.as_type_tag(sub), ctx.registry.as_type_tag(sup))
        {
            if tag_sub == "type.universe" && tag_sup == "type.universe" {
                let level_sub = sub.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
                let level_sup = sup.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
                if level_sub <= level_sup {
                    return Ok(());
                } else {
                    return Err(LangError::engine(format!(
                        "typecheck: Type_{} is not a subtype of Type_{} (cumulativity requires n ≤ m)",
                        level_sub, level_sup
                    )));
                }
            }
            // For same-tag non-universe types, use structural equality
            // (covariant structural subtyping is a future extension).
            if tag_sub == tag_sup {
                return self.unify(sub, sup, ctx);
            }
        }
        // Default: not a subtype.
        Err(LangError::engine(format!(
            "typecheck: {:?} is not a subtype of {:?}",
            sub, sup
        )))
    }

    /// Unify two type values (structural equality + metavariable solving +
    /// recursive structural unification of tagged type values).
    ///
    /// If both sides are equal, succeeds. If one side is an unsolved
    /// metavariable (`{"__meta__": id}`), solves it to the other side
    /// (after an occurs-check). If both sides are tagged type values from
    /// the same registry with the same tag, recursively unifies their
    /// payload fields (ADR-0054 legacy migration). Otherwise returns an
    /// engine error.
    fn unify(&self, a: &Value, b: &Value, ctx: &mut TypecheckContext) -> Result<(), LangError> {
        if a == b {
            return Ok(());
        }
        if let Some(id) = extract_meta_id(a) {
            if self.occurs_in(id as usize, b) {
                return Err(LangError::engine(format!(
                    "typecheck: occurs-check failed for meta {}",
                    id
                )));
            }
            ctx.solve_meta(id, b.clone());
            return Ok(());
        }
        if let Some(id) = extract_meta_id(b) {
            if self.occurs_in(id as usize, a) {
                return Err(LangError::engine(format!(
                    "typecheck: occurs-check failed for meta {}",
                    id
                )));
            }
            ctx.solve_meta(id, a.clone());
            return Ok(());
        }
        // Recursive structural unification of tagged type values with the
        // same tag from the same registry (ADR-0054 legacy migration).
        // This lets `unify(type.function{param=type.number, body=type.string},
        // type.function{param=type.number, body=type.string})` succeed by
        // recursively unifying the `param` and `body` payload fields.
        if let (Some(tag_a), Some(tag_b)) =
            (ctx.registry.as_type_tag(a), ctx.registry.as_type_tag(b))
        {
            if tag_a == tag_b {
                // Universe cumulativity (ADR-0054): two `type.universe` values
                // unify iff their levels are equal. The cumulative hierarchy
                // is `Type_0 ⊆ Type_1 ⊆ ...`; for unification we require
                // equality (mirrors the legacy `l1.le(l2) && l2.le(l1)`).
                // Subtyping (`Type_n ⊆ Type_m` if `n ≤ m`) is reserved for a
                // future subtyping judgment.
                if tag_a == "type.universe" {
                    let level_a = a.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
                    let level_b = b.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
                    if level_a == level_b {
                        return Ok(());
                    } else {
                        return Err(LangError::engine(format!(
                            "typecheck: cannot unify Type_{} with Type_{}",
                            level_a, level_b
                        )));
                    }
                }
                // Row-polymorphic record unification (ADR-0074 ID.7):
                // `type.record` values may carry a `rest` field holding a
                // row variable (metavariable). When present, the record is
                // OPEN and `rest` captures extra fields during unification.
                // When absent, the record is CLOSED (exact field match).
                // This special case must run BEFORE the general same-tag
                // structural unification, which would require identical
                // payload keys.
                if tag_a == "type.record" {
                    return self.unify_record(a, b, ctx);
                }
                // Row-polymorphic effect row unification (ADR-0054 /
                // ADR-0076): `type.effects` values carry an `ops` array and
                // an optional `rest` row variable. This special case must run
                // BEFORE the general same-tag structural unification, which
                // would try to unify the `ops` arrays as JSON arrays and the
                // `rest` row variables as plain values.
                if tag_a == "type.effects" {
                    return self.unify_effects(a, b, ctx);
                }
                // Effect row unification (ADR-0054 / ADR-0076):
                // `type.function` values may carry an `effects` field holding
                // a `type.effects` value (an effect row). When both sides have
                // an `effects` field, the rows are unified via
                // [`unify_effects`] (row-polymorphic). When one or both sides
                // lack `effects`, effect unification is skipped (backward
                // compatible — the function's effects are unknown/untracked).
                // The `param` and `body` fields are always unified
                // structurally. This special case must run BEFORE the general
                // same-tag structural unification, which would treat a
                // missing `effects` field as `Null` and fail.
                if tag_a == "type.function" {
                    let effects_a = a.get("effects");
                    let effects_b = b.get("effects");
                    let param_a = a.get("param").unwrap_or(&Value::Null);
                    let param_b = b.get("param").unwrap_or(&Value::Null);
                    self.unify(param_a, param_b, ctx)?;
                    let body_a = a.get("body").unwrap_or(&Value::Null);
                    let body_b = b.get("body").unwrap_or(&Value::Null);
                    self.unify(body_a, body_b, ctx)?;
                    // Unify effects only when both sides carry an effects field.
                    if let (Some(ea), Some(eb)) = (effects_a, effects_b) {
                        self.unify_effects(ea, eb, ctx)?;
                    }
                    return Ok(());
                }
                // Both are type values with the same tag — unify payload
                // fields.  We use `get` (not `as_object`) because the type
                // value is always a JSON object (the registry guarantees
                // this).  We still guard with `ok_or_else` so a malformed
                // (non-object) type value surfaces as a `LangError` instead
                // of panicking.
                let obj_a = a
                    .as_object()
                    .ok_or_else(|| LangError::engine("typecheck: type value is not an object"))?;
                let obj_b = b
                    .as_object()
                    .ok_or_else(|| LangError::engine("typecheck: type value is not an object"))?;
                // Collect field keys from both objects, excluding the
                // `__tag__` and `__registry__` structural keys and the
                // `body_closure` field (which is an engine-internal closure
                // marker for dependent Pi/Sigma types — it cannot be
                // structurally unified; use the `unify_pi` judgment op for
                // full closure-based unification, ADR-0054).
                let keys: std::collections::BTreeSet<String> = obj_a
                    .keys()
                    .filter(|k| *k != "__tag__" && *k != "__registry__" && *k != "body_closure")
                    .chain(obj_b.keys().filter(|k| {
                        *k != "__tag__" && *k != "__registry__" && *k != "body_closure"
                    }))
                    .cloned()
                    .collect();
                for key in keys {
                    let va = obj_a.get(&key).unwrap_or(&Value::Null);
                    let vb = obj_b.get(&key).unwrap_or(&Value::Null);
                    self.unify(va, vb, ctx)?;
                }
                return Ok(());
            }
        }
        Err(LangError::engine(format!(
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
            return Err(LangError::engine("typecheck: empty ast"));
        }
        // The root node is the LAST node in the arena (Pratt convention).
        let root_ref = AstRef::new(req.ast.len() - 1);
        let mut ctx = TypecheckContext::new(&self.registry_id);
        let typ = self.synth_node(lang, &req.ast, root_ref, &mut ctx)?;
        Ok(TypecheckResult {
            typ,
            diagnostics: ctx.diagnostics,
        })
    }

    fn box_clone(&self) -> Box<dyn TypecheckEngine> {
        Box::new(Self {
            eval_engine: self.eval_engine.box_clone(),
            registry_id: self.registry_id.clone(),
            pure: self.pure,
            modules: self.modules.clone(),
        })
    }

    fn with_modules(
        &self,
        modules: Rc<dyn handl_platform::ModuleLookup>,
    ) -> Box<dyn TypecheckEngine> {
        Box::new(Self {
            eval_engine: self.eval_engine.with_modules(modules.clone()),
            registry_id: self.registry_id.clone(),
            pure: self.pure,
            modules: Some(modules),
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
    /// Non-fatal diagnostics accumulated during typechecking (ADR-0068:
    /// runtime-assert requirements from `Blocked` predicate annotations).
    diagnostics: Vec<TypecheckDiagnostic>,
}

impl TypecheckContext {
    /// Create a fresh context with the given registry id.
    fn new(registry_id: &str) -> Self {
        Self {
            metas: Vec::new(),
            gamma: Vec::new(),
            results: HashMap::new(),
            registry: TypeRegistry::new(registry_id),
            diagnostics: Vec::new(),
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

    /// Mark the current length of Γ for later restoration (scoped closure
    /// evaluation, ADR-0054 dependent types). Returns a mark that can be
    /// passed to [`restore`](Self::restore) to pop all bindings added since.
    fn mark(&self) -> usize {
        self.gamma.len()
    }

    /// Restore Γ to the length recorded by a prior [`mark`](Self::mark).
    /// This pops all bindings added after the mark, implementing scoped
    /// context management for closure application (ADR-0054).
    fn restore(&mut self, mark: usize) {
        self.gamma.truncate(mark);
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

/// Check that no node in the `ast` subtree rooted at `root` has an eval
/// scheme invoking a `Foreign` op (ADR-0054 pure-eval path). Returns
/// `Err` if any `Foreign` name is found.
///
/// Walks every node in the subtree, looks up its eval scheme in
/// `lang.eval`, and collects `Foreign` names via
/// [`handl_language::foreign_names`]. This is a *static* purity check —
/// it inspects the scheme data, not runtime behavior — so it catches an
/// effectful call before the bound eval engine could execute it.
fn check_pure(lang: &Language, ast: &Ast, root: AstRef) -> Result<(), LangError> {
    let mut foreigns = std::collections::BTreeSet::new();
    collect_subtree_foreigns(lang, ast, root, &mut foreigns);
    if foreigns.is_empty() {
        Ok(())
    } else {
        Err(LangError::engine(format!(
            "typecheck: type expression invokes effectful foreign ops {:?} (pure-eval path, ADR-0054)",
            foreigns
        )))
    }
}

/// Recursively walk the subtree rooted at `root`, collecting `Foreign`
/// names from each node's eval scheme (looked up in `lang.eval` by the
/// node's `kind`).
fn collect_subtree_foreigns(
    lang: &Language,
    ast: &Ast,
    root: AstRef,
    out: &mut std::collections::BTreeSet<String>,
) {
    if let Some(node) = ast.get(root) {
        if let Some(eval_def) = lang.eval.get_by_identity(&node.kind) {
            for name in handl_language::foreign_names(&eval_def.scheme) {
                out.insert(name);
            }
        }
        // Recurse into children.
        if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
            for child_ref in children {
                if let Some(idx) = child_ref.as_u64() {
                    collect_subtree_foreigns(lang, ast, AstRef::new(idx as usize), out);
                }
            }
        }
    }
}

/// Get the [`AstRef`] of child `idx` of the node at `node_ref`.
///
/// Reads `node.payload["children"][idx]` as a u64 arena index.
fn child_ref(ast: &Ast, node_ref: AstRef, idx: u64) -> Result<AstRef, LangError> {
    let node: &AstNode = ast.get(node_ref).ok_or_else(|| {
        LangError::engine(format!(
            "typecheck: child_ref: node index {} out of bounds",
            node_ref.index()
        ))
    })?;
    let children = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: child_ref: node kind '{}' has no children array",
                node.kind
            ))
        })?;
    let child_idx = children
        .get(idx as usize)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            LangError::engine(format!(
                "typecheck: child_ref: child index {} out of bounds or not a u64",
                idx
            ))
        })?;
    Ok(AstRef::new(child_idx as usize))
}

// --- ADR-0081 data-driven child roles --------------------------------------
//
// Engines must resolve AST child positions by **named role** from
// `AstShapeDef` (`data/ast.yaml`), never by `kind ==` shape heuristics or
// bare positional indices. Each helper falls back to the legacy positional
// behavior only when a node's shape declares no roles (opaque shapes), so
// the canonical language and role-declaring languages behave identically.

/// Resolve a child role to its positional index for `node_kind`.
///
/// `None` = the node kind has no shape or declares no such role; callers
/// keep their positional fallback.
fn role_index_of(lang: &Language, node_kind: &str, role: &str) -> Option<usize> {
    lang.child_role_index(node_kind, role).ok().flatten()
}

/// Whether `node_kind`'s shape declares the given child role.
fn role_declared(lang: &Language, node_kind: &str, role: &str) -> bool {
    role_index_of(lang, node_kind, role).is_some()
}

/// Project the child arena ref at `role` from `node`'s payload children.
///
/// `None` = no such role declared or no children payload; callers fall back
/// to the positional index.
fn role_child_ref(
    lang: &Language,
    node: &AstNode,
    role: &str,
) -> Option<AstRef> {
    let idx = role_index_of(lang, &node.kind, role)?;
    let arena = node
        .payload
        .get("children")?
        .as_array()?
        .get(idx)?
        .as_u64()?;
    Some(AstRef::new(arena as usize))
}

/// Whether `node` is a binding (assign-like) node: its shape declares a
/// `target` role. Falls back to the legacy kind check for role-less shapes.
fn is_assign_node(lang: &Language, node: &AstNode) -> bool {
    role_declared(lang, &node.kind, "target") || node.kind == "assign"
}

/// Whether `node` is a return node: its shape declares a `value` role but no
/// `target` role (a binding declares both). Falls back to the legacy kind
/// check for role-less shapes.
fn is_return_node(lang: &Language, node: &AstNode) -> bool {
    (role_declared(lang, &node.kind, "value") && !role_declared(lang, &node.kind, "target"))
        || node.kind == "return"
}

/// Whether `node` is a type annotation: its shape declares an `annot` role.
/// Falls back to the legacy kind check for role-less shapes.
fn is_type_annot(lang: &Language, node: &AstNode) -> bool {
    role_declared(lang, &node.kind, "annot") || node.kind == "type-annot"
}

/// Whether `node` is a block: its shape declares a `statements` role.
/// Falls back to the legacy kind check for role-less shapes.
fn is_block_node(lang: &Language, node: &AstNode) -> bool {
    role_declared(lang, &node.kind, "statements") || node.kind == "block"
}

/// Whether `node` is a paren wrapper: its shape declares a `body` role.
/// Falls back to the legacy kind check for role-less shapes.
fn is_paren_node(lang: &Language, node: &AstNode) -> bool {
    role_declared(lang, &node.kind, "body") || node.kind == "paren"
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

/// Interpret a type-expression AST node into a concrete type value via
/// the registry.
///
/// Supported forms:
/// - `field-access` (`type.number`, `type.string`, …) → atomic `type.{field}`
/// - `paren` (`(T)`) → unwrap and interpret the single child
/// - `function` (`(T) -> U`) → `type.function{param: T, body: U}`
/// - `array` (`[T]`) → `type.array{element: T}`
///
/// Compound forms recurse on their children, so `(type.number) -> [type.string]`
/// produces `type.function{param: type.number, body: type.array{element: type.string}}`.
fn interpret_type_expr(
    lang: &Language,
    ast: &Ast,
    node: &handl_language::AstNode,
    registry: &TypeRegistry,
) -> Result<Value, LangError> {
    match node.kind.as_str() {
        "field-access" => {
            let children = node
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine("interpret_type_expr: field-access has no children")
                })?;
            if children.len() < 2 {
                return Err(LangError::engine(
                    "interpret_type_expr: field-access needs 2 children",
                ));
            }
            let field_idx = children[1].as_u64().ok_or_else(|| {
                LangError::engine("interpret_type_expr: field child index is not a u64")
            })? as usize;
            let field_node = ast.get(AstRef::new(field_idx)).ok_or_else(|| {
                LangError::engine(format!(
                    "interpret_type_expr: field node {} out of bounds",
                    field_idx
                ))
            })?;
            let field = field_node
                .payload
                .get("name")
                .and_then(|v| v.as_str())
                .or_else(|| field_node.payload.get("value").and_then(|v| v.as_str()))
                .or_else(|| field_node.payload.as_str().map(|s| s))
                .ok_or_else(|| {
                    LangError::engine_node(
                        "interpret_type_expr: field node has no name/value string",
                        field_node,
                    )
                })?;
            let tag = format!("type.{}", field);
            registry.construct(&tag, &Value::Null).ok_or_else(|| {
                LangError::engine(format!(
                    "interpret_type_expr: failed to construct type '{}'",
                    tag
                ))
            })
        }
        "paren" => {
            // Grouping: unwrap the single child and interpret it.
            let child_idx = first_child_idx(node)?;
            let child = ast.get(AstRef::new(child_idx)).ok_or_else(|| {
                LangError::engine(format!(
                    "interpret_type_expr: paren child {} out of bounds",
                    child_idx
                ))
            })?;
            interpret_type_expr(lang, ast, child, registry)
        }
        "function" => {
            // `(T) -> U` → type.function{param: T, body: U}.
            let children = node
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine("interpret_type_expr: function has no children")
                })?;
            if children.len() < 2 {
                return Err(LangError::engine(
                    "interpret_type_expr: function needs 2 children (param, body)",
                ));
            }
            // Resolve param/body by child role (ADR-0081), falling back to
            // the positional children for role-less shapes.
            let param_idx = role_child_ref(lang, node, "params")
                .map(|r| r.index())
                .or_else(|| children[0].as_u64().map(|i| i as usize))
                .ok_or_else(|| {
                    LangError::engine("interpret_type_expr: param child index is not a u64")
                })?;
            let body_idx = role_child_ref(lang, node, "body")
                .map(|r| r.index())
                .or_else(|| children[1].as_u64().map(|i| i as usize))
                .ok_or_else(|| {
                    LangError::engine("interpret_type_expr: body child index is not a u64")
                })?;
            let param_node = ast.get(AstRef::new(param_idx)).ok_or_else(|| {
                LangError::engine(format!(
                    "interpret_type_expr: param node {} out of bounds",
                    param_idx
                ))
            })?;
            let body_node = ast.get(AstRef::new(body_idx)).ok_or_else(|| {
                LangError::engine(format!(
                    "interpret_type_expr: body node {} out of bounds",
                    body_idx
                ))
            })?;
            // The param child is a paren wrapper (paren{body}) around the
            // param type expr; unwrap by role when declared.
            let param_type = if is_paren_node(lang, param_node) {
                let inner_idx = role_child_ref(lang, param_node, "body")
                    .map(|r| r.index())
                    .or_else(|| first_child_idx(param_node).ok())
                    .ok_or_else(|| {
                        LangError::engine("interpret_type_expr: param has no body child")
                    })?;
                let inner = ast.get(AstRef::new(inner_idx)).ok_or_else(|| {
                    LangError::engine(format!(
                        "interpret_type_expr: param paren child {} out of bounds",
                        inner_idx
                    ))
                })?;
                interpret_type_expr(lang, ast, inner, registry)?
            } else {
                interpret_type_expr(lang, ast, param_node, registry)?
            };
            let body_type = interpret_type_expr(lang, ast, body_node, registry)?;
            let payload = serde_json::json!({"param": param_type, "body": body_type});
            registry
                .construct("type.function", &payload)
                .ok_or_else(|| {
                    LangError::engine_node(
                        "interpret_type_expr: failed to construct type.function",
                        body_node,
                    )
                })
        }
        "array" => {
            // `[T]` → type.array{element: T}.
            let child_idx = first_child_idx(node)?;
            let child = ast.get(AstRef::new(child_idx)).ok_or_else(|| {
                LangError::engine(format!(
                    "interpret_type_expr: array child {} out of bounds",
                    child_idx
                ))
            })?;
            let element_type = interpret_type_expr(lang, ast, child, registry)?;
            let payload = serde_json::json!({"element": element_type});
            registry.construct("type.array", &payload).ok_or_else(|| {
                LangError::engine("interpret_type_expr: failed to construct type.array")
            })
        }
        "call" => {
            // Meta-predicate type expressions: `Total(A -> B)` or
            // `Pure(A -> B)`. The callee (child 0) is a `var` naming the
            // predicate (e.g. `Total`) or a `field-access` like
            // `type.Total`. The argument (child 1) is the function type
            // (e.g. `(type.number) -> type.number`).
            //
            // We construct `type.Total{arg: <fn-type>}` or
            // `type.Pure{arg: <fn-type>}`. The typechecker's
            // `synth_type_annot` detects these tags and dispatches to the
            // meta-property checker (totality / purity).
            let children = node
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine("interpret_type_expr: call has no children array")
                })?;
            if children.len() < 2 {
                return Err(LangError::engine(
                    "interpret_type_expr: call needs 2 children (callee, arg)",
                ));
            }
            let callee_idx = children[0].as_u64().ok_or_else(|| {
                LangError::engine("interpret_type_expr: call callee index is not a u64")
            })? as usize;
            let arg_idx = children[1].as_u64().ok_or_else(|| {
                LangError::engine("interpret_type_expr: call arg index is not a u64")
            })? as usize;
            let callee_node = ast.get(AstRef::new(callee_idx)).ok_or_else(|| {
                LangError::engine(format!(
                    "interpret_type_expr: call callee node {} out of bounds",
                    callee_idx
                ))
            })?;

            // Resolve the callee to a meta-predicate type tag.
            let meta_tag = resolve_meta_predicate_tag(ast, callee_node);
            match meta_tag {
                Some(tag) => {
                    let arg_node = ast.get(AstRef::new(arg_idx)).ok_or_else(|| {
                        LangError::engine(format!(
                            "interpret_type_expr: call arg node {} out of bounds",
                            arg_idx
                        ))
                    })?;
                    let arg_type = interpret_type_expr(lang, ast, arg_node, registry)?;
                    let payload = serde_json::json!({"arg": arg_type});
                    registry.construct(tag, &payload).ok_or_else(|| {
                        LangError::engine(format!(
                            "interpret_type_expr: failed to construct '{}'",
                            tag
                        ))
                    })
                }
                None => Err(LangError::engine(format!(
                    "interpret_type_expr: unsupported type-expr kind '{}'",
                    node.kind
                ))),
            }
        }
        _ => Err(LangError::engine(format!(
            "interpret_type_expr: unsupported type-expr kind '{}'",
            node.kind
        ))),
    }
}

/// Resolve a callee AST node to a meta-predicate type tag.
///
/// Recognizes `var("Total")` / `var("Pure")` and `field-access` producing
/// `type.Total` / `type.Pure`. Returns `None` if the callee does not name a
/// known meta-predicate.
fn resolve_meta_predicate_tag(
    ast: &Ast,
    node: &handl_language::AstNode,
) -> Option<&'static str> {
    match node.kind.as_str() {
        "var" => {
            let name = node.payload.as_str()?;
            match name {
                "Total" => Some("type.Total"),
                "Pure" => Some("type.Pure"),
                _ => None,
            }
        }
        "field-access" => {
            // `type.Total` or `type.Pure` — the field name is in the payload
            // (`field` key) or in a child node (children[1]).
            let field: String = node
                .payload
                .get("field")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    let children = node.payload.get("children")?.as_array()?;
                    let field_idx = children.get(1)?.as_u64()? as usize;
                    let field_node = ast.get(AstRef::new(field_idx))?;
                    field_node
                        .payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .or_else(|| field_node.payload.get("value").and_then(|v| v.as_str()))
                        .or_else(|| field_node.payload.as_str())
                        .map(|s| s.to_string())
                })?;
            match field.as_str() {
                "Total" => Some("type.Total"),
                "Pure" => Some("type.Pure"),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Extract the first child index from a node's `children` payload array.
fn first_child_idx(node: &handl_language::AstNode) -> Result<usize, LangError> {
    let children = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .ok_or_else(|| LangError::engine("interpret_type_expr: node has no children array"))?;
    if children.is_empty() {
        return Err(LangError::engine(
            "interpret_type_expr: node has no children",
        ));
    }
    children[0]
        .as_u64()
        .map(|i| i as usize)
        .ok_or_else(|| LangError::engine("interpret_type_expr: child index is not a u64"))
}

// ---------------------------------------------------------------------------
// ADR-0082 step 3: Termination metadata (data-driven structural recursion)
// ---------------------------------------------------------------------------

/// Termination info extracted from a `function` node's payload.
///
/// When `data` desugars, it generates a recursor whose inner function
/// carries a `termination` payload field declaring structural recursion:
/// the function decreases on parameter `decreasing_arg` via the field
/// `decreasing_field`. The typechecker reads this metadata to recognize
/// structural recursion without hardcoded name matching (e.g. no
/// `kind == "Type.rec"` check).
#[derive(Debug, Clone)]
pub struct TerminationInfo {
    /// The parameter index that decreases (0-based).
    pub decreasing_arg: usize,
    /// The field name used to decrease (e.g. "pred" for Nat).
    pub decreasing_field: String,
}

/// Read termination metadata from a `function` node's payload.
///
/// Returns `Some(TerminationInfo)` if the node has a `termination` field
/// in its payload, or `None` if the field is absent (non-recursive
/// functions, or functions not generated by `data` desugaring).
///
/// This is the data-driven entry point for termination checking. When a
/// `Total` predicate lands (Phase 7), the termination checker will call
/// this helper to determine whether a function is structurally recursive
/// and on which argument/field, without matching on function names.
pub fn read_termination_metadata(node: &handl_language::AstNode) -> Option<TerminationInfo> {
    let term = node.payload.get("termination")?;
    let decreasing_arg = term
        .get("decreasing_arg")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)?;
    let decreasing_field = term
        .get("decreasing_field")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())?;
    Some(TerminationInfo {
        decreasing_arg,
        decreasing_field,
    })
}

#[cfg(test)]
mod tests;
