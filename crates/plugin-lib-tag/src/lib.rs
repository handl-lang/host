//! # handl-plugin-lib-tag
//!
//! The **tagged-values** capability as a composable `plugin-lib-*` leaf crate
//! (ADR-0055 — Tagged Values).
//!
//! Tagged values are the host primitive that lets the typechecker, units,
//! capabilities, and provenance share one value space without collision
//! (ADR-0032 "types are values"). A tagged value is a JSON object
//! `{"__tag__": <name>, ...payload}` minted by the host. Tagging is a **host
//! capability**, not a user-constructible convention: the `__tag__` key is
//! reserved (the eval engines reject it in user object literals), so the
//! only way a value acquires a `__tag__` key is through this plugin's
//! `tag.tag` host function. User code cannot forge a tagged value.
//!
//! This crate exposes the `tag` module with seven callables:
//!
//! | Callable | Role |
//! |---|---|
//! | `tag.tag(name, payload)` | **mint** a tagged value (host capability) |
//! | `tag.match_tag(v, cases)` | **dispatch** on the tag of `v`; cases bind the payload |
//! | `tag.untag(v)` | return the payload of a tagged value (inspection) |
//! | `tag.tag_of(v)` | return the tag name of a tagged value (inspection) |
//! | `tag.is_tag(v)` | test whether `v` is a tagged value (inspection) |
//! | `tag.construct(tag_name, payload)` | **mint** a registry-scoped type value (ADR-0054) |
//! | `tag.match_type(v, cases)` | **dispatch** on the type tag of `v` (ADR-0054) |
//!
//! `tag.construct` and `tag.match_type` are the user-facing surface of the
//! ADR-0054 judgment ops `construct` and `match_type`: they are backed by a
//! [`TypeRegistry`] with registry id `"handl.canonical"`.
//!
//! `tag.tag` is the minting capability (host-secured); the inspection ops are
//! available to any code that holds a tagged value — matching the capability
//! discipline of ADR-0055 §1 (`tag` is restricted; `match_tag` is not).
//!
//! This crate is a leaf: it depends on `handl-language` (the value helpers +
//! reserved-key predicate), `handl-platform` (`Function`/`Module`), and
//! `handl-plugin-module` (`ModuleRuntime` registration) only — no engine
//! crates, no `handl-runtime`.

#![warn(missing_docs)]

use handl_language::{as_tag, is_tag_value, tag_value, Value};
use handl_platform::{EvalContext, EvalError, Function, Module};

/// The host type registry (ADR-0054).
pub mod registry;
pub use registry::TypeRegistry;

pub mod plugin;
pub use plugin::LibraryPlugin;

#[cfg(test)]
mod tests;

/// The reserved default-case key in `tag.match_tag`'s `cases` object.
///
/// A case keyed `"_"` is the fallback dispatched when `v`'s tag has no
/// explicit case.
pub const DEFAULT_CASE_KEY: &str = "_";

// ---------------------------------------------------------------------------
// tag.tag — mint a tagged value (host capability)
// ---------------------------------------------------------------------------

/// `tag.tag(name, payload)` — mint a tagged value (ADR-0055 §1, §2).
///
/// Mints `{"__tag__": name, ...payload}`. The `payload` object's fields
/// become the tagged value's payload; a non-object payload (or `null`) is
/// treated as empty. Returns an error if `name` is not a string, if
/// `payload` is not an object/null, or if `payload` already contains a
/// reserved `__tag__` key (a minted value cannot be re-tagged through its
/// payload — forgery prevention, ADR-0055 §3).
#[derive(Clone, Copy)]
pub struct Tag;

impl Function for Tag {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "tag.tag: expected 2 arguments (name, payload), got {}",
                args.len()
            )));
        }
        let name = args[0]
            .as_str()
            .ok_or_else(|| EvalError::Other("tag.tag: name must be a string".to_string()))?;
        tag_value(name, &args[1]).ok_or_else(|| {
            EvalError::Other(
                "tag.tag: payload must be an object (or null) without a reserved __tag__ key"
                    .to_string(),
            )
        })
    }
}

// ---------------------------------------------------------------------------
// tag.match_tag — dispatch on the tag of a value
// ---------------------------------------------------------------------------

/// `tag.match_tag(v, cases)` — dispatch on the tag of `v` (ADR-0055 §1).
///
/// `cases` is an object mapping tag names to case arrows (arrow-values). The
/// tag of `v` is read; the matching case arrow is invoked with the tagged
/// value's payload. If no case matches, the `"_"` default case (if present)
/// is invoked. If neither exists, returns `null`.
///
/// **Binding the payload fields.** If the matched case arrow has multiple
/// parameters and the payload is an object, each parameter is bound to the
/// corresponding payload field by name (mirroring the `loop_step`
/// destructuring in the eval engine): `match_tag({"__tag__":"unit","m":5,"s":1},
/// {"unit": (m, s) ~> ...})` binds `m=5, s=1`. Otherwise the payload is passed
/// as a single argument.
///
/// This is the inspection op: any code holding a tagged value can read its
/// tag and fields (ADR-0055 §1). It is not a restricted capability.
#[derive(Clone, Copy)]
pub struct MatchTag;

impl Function for MatchTag {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "tag.match_tag: expected 2 arguments (value, cases), got {}",
                args.len()
            )));
        }
        let v = &args[0];
        let cases = args[1]
            .as_object()
            .ok_or_else(|| EvalError::Other("tag.match_tag: cases must be an object".to_string()))?;

        // Resolve the case arrow: the tag's case, else the "_" default.
        let case = match as_tag(v) {
            Some(tag) => match cases.get(tag) {
                Some(c) => Some(c),
                None => cases.get(DEFAULT_CASE_KEY),
            },
            None => cases.get(DEFAULT_CASE_KEY),
        };
        let Some(case) = case else {
            // No matching case and no default — return null.
            return Ok(Value::Null);
        };

        // Build the call arguments from the payload (v without __tag__).
        let payload = payload_of(v);
        let call_args = destructure_args(case, &payload);

        // Invoke the case arrow via the engine's value-callable bridge.
        match ctx.try_call_value(case, &call_args) {
            Some(result) => result,
            None => {
                // No value-callable wired (e.g. the case isn't an arrow-value).
                // If the case is a non-arrow value, return it as the result.
                if is_arrow_value(case) {
                    Err(EvalError::Other(
                        "tag.match_tag: case is an arrow-value but no value callable is wired"
                            .to_string(),
                    ))
                } else {
                    Ok(case.clone())
                }
            }
        }
    }
}

/// Extract the payload of a tagged value: a clone of `v` with the `__tag__`
/// key removed. For a non-tagged value, returns `v` unchanged.
fn payload_of(v: &Value) -> Value {
    if let Some(obj) = v.as_object() {
        if obj.contains_key(handl_language::TAG_VALUE_KEY) {
            let mut map = obj.clone();
            map.remove(handl_language::TAG_VALUE_KEY);
            return Value::Object(map);
        }
    }
    v.clone()
}

/// Whether `v` is an arrow-value (`{"__arrow__": true, ...}`).
fn is_arrow_value(v: &Value) -> bool {
    v.get("__arrow__")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Build the argument list for invoking `case` with `payload`.
///
/// If `case` is an arrow-value with multiple named params and `payload` is an
/// object, bind each param to the corresponding payload field by name
/// (missing fields default to `null`). Otherwise pass `payload` as a single
/// argument. Mirrors the `loop_step` destructuring in the tree-walk engine.
fn destructure_args(case: &Value, payload: &Value) -> Vec<Value> {
    let Some(params) = case
        .get("params")
        .and_then(|p| p.as_array())
    else {
        return vec![payload.clone()];
    };
    if params.len() <= 1 {
        return vec![payload.clone()];
    }
    let Some(obj) = payload.as_object() else {
        return vec![payload.clone()];
    };
    params
        .iter()
        .map(|p| {
            let name = p.as_str().unwrap_or("_");
            obj.get(name).cloned().unwrap_or(Value::Null)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Inspection ops — untag / tag_of / is_tag
// ---------------------------------------------------------------------------

/// `tag.untag(v)` — return the payload of a tagged value (ADR-0055 §1).
///
/// Returns `v` with the `__tag__` key removed. Returns `null` if `v` is not a
/// tagged value.
#[derive(Clone, Copy)]
pub struct Untag;

impl Function for Untag {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = require_one_arg(args, "tag.untag")?;
        if !is_tag_value(v) {
            return Ok(Value::Null);
        }
        Ok(payload_of(v))
    }
}

/// `tag.tag_of(v)` — return the tag name of a tagged value (ADR-0055 §1).
///
/// Returns the tag name string, or `null` if `v` is not a tagged value.
#[derive(Clone, Copy)]
pub struct TagOf;

impl Function for TagOf {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = require_one_arg(args, "tag.tag_of")?;
        Ok(match as_tag(v) {
            Some(name) => Value::from(name),
            None => Value::Null,
        })
    }
}

/// `tag.is_tag(v)` — test whether `v` is a tagged value (ADR-0055 §1).
///
/// Returns `true` iff `v` is a tagged value minted by the host `tag`
/// capability (i.e. an object carrying a string `__tag__` key).
#[derive(Clone, Copy)]
pub struct IsTag;

impl Function for IsTag {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = require_one_arg(args, "tag.is_tag")?;
        Ok(Value::from(is_tag_value(v)))
    }
}

/// Require exactly one argument, with a function-name prefix for errors.
fn require_one_arg<'a>(args: &'a [Value], fn_name: &str) -> Result<&'a Value, EvalError> {
    args.first().ok_or_else(|| {
        EvalError::Other(format!(
            "{fn_name}: expected 1 argument, got {}",
            args.len()
        ))
    })
}

// ---------------------------------------------------------------------------
// tag.construct / tag.match_type — host type registry (ADR-0054)
// ---------------------------------------------------------------------------

/// The registry id of the canonical handl typechecker, used by the
/// `tag.construct` / `tag.match_type` host functions.
pub const CANONICAL_REGISTRY_ID: &str = "handl.canonical";

/// `tag.construct(tag_name, payload)` — mint a registry-scoped type value
/// (ADR-0054 §"construct").
///
/// Mints `{"__tag__": tag_name, "__registry__": "handl.canonical",
/// ...payload}` via the canonical [`TypeRegistry`]. This is the
/// `construct` judgment op of ADR-0054: the `tag` op (ADR-0055) called
/// against the host type registry. Returns an error if `tag_name` is not a
/// string, if `payload` is not an object/null, or if `payload` smuggles a
/// reserved `__tag__` or `__registry__` key (forgery prevention).
#[derive(Clone, Copy)]
pub struct Construct;

impl Function for Construct {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "tag.construct: expected 2 arguments (tag_name, payload), got {}",
                args.len()
            )));
        }
        let tag_name = args[0]
            .as_str()
            .ok_or_else(|| EvalError::Other("tag.construct: tag_name must be a string".to_string()))?;
        let registry = TypeRegistry::new(CANONICAL_REGISTRY_ID);
        registry.construct(tag_name, &args[1]).ok_or_else(|| {
            EvalError::Other(
                "tag.construct: payload must be an object (or null) without reserved __tag__/__registry__ keys"
                    .to_string(),
            )
        })
    }
}

/// `tag.match_type(v, cases)` — dispatch on the type tag of `v` (ADR-0054
/// §"match_type").
///
/// `cases` is an object mapping type tag names to case values. If `v` is a
/// type value minted by the canonical registry whose tag has an explicit
/// case, that case is returned. Otherwise returns `null` — there is no
/// default-case fallback (the caller handles non-matches, per ADR-0054).
/// This is `match_tag` on type tags scoped to the canonical registry.
#[derive(Clone, Copy)]
pub struct MatchType;

impl Function for MatchType {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "tag.match_type: expected 2 arguments (value, cases), got {}",
                args.len()
            )));
        }
        let registry = TypeRegistry::new(CANONICAL_REGISTRY_ID);
        Ok(registry.match_type(&args[0], &args[1]).unwrap_or(Value::Null))
    }
}

// ---------------------------------------------------------------------------
// Module + reserved-key re-export
// ---------------------------------------------------------------------------

static TAG: Tag = Tag;
static MATCH_TAG: MatchTag = MatchTag;
static UNTAG: Untag = Untag;
static TAG_OF: TagOf = TagOf;
static IS_TAG: IsTag = IsTag;
static CONSTRUCT: Construct = Construct;
static MATCH_TYPE: MatchType = MatchType;

/// The `tag` module — tagged-value host functions (ADR-0055) plus the
/// host type registry surface (ADR-0054).
///
/// Provides `tag.tag`, `tag.match_tag`, `tag.untag`, `tag.tag_of`,
/// `tag.is_tag`, `tag.construct`, and `tag.match_type` callable from handl
/// expressions.
#[derive(Clone, Copy)]
pub struct TagModule;

impl Module for TagModule {
    fn name(&self) -> &str {
        "tag"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "tag" => Some(&TAG),
            "match_tag" => Some(&MATCH_TAG),
            "untag" => Some(&UNTAG),
            "tag_of" => Some(&TAG_OF),
            "is_tag" => Some(&IS_TAG),
            "construct" => Some(&CONSTRUCT),
            "match_type" => Some(&MATCH_TYPE),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![
            ("tag", &TAG as &dyn Function),
            ("match_tag", &MATCH_TAG as &dyn Function),
            ("untag", &UNTAG as &dyn Function),
            ("tag_of", &TAG_OF as &dyn Function),
            ("is_tag", &IS_TAG as &dyn Function),
            ("construct", &CONSTRUCT as &dyn Function),
            ("match_type", &MATCH_TYPE as &dyn Function),
        ]
    }
}

/// Re-export the reserved-key predicate for hosts/engines that want to
/// enforce the `__tag__` reservation at construction sites beyond the eval
/// engines (e.g. a future typecheck host).
pub use handl_language::is_reserved_value_key;
