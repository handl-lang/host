//! # handl-lib-cmp
//!
//! Comparison and ordering utilities for the handl runtime.
//!
//! Provides the `cmp` module — `cmp.eq`, `cmp.ne`, `cmp.lt`, `cmp.le`,
//! `cmp.gt`, `cmp.ge`, `cmp.compare`, `cmp.min`, `cmp.max`, `cmp.clamp`,
//! `cmp.between`, `cmp.in_range`, `cmp.is_sorted`, `cmp.is_sorted_desc`,
//! `cmp.sort`, `cmp.sort_desc`, `cmp.sort_by`, `cmp.sort_by_desc`,
//! `cmp.min_by`, `cmp.max_by`.
//!
//! These are the functions that the `cmp` grammar sugars (`==`, `!=`, `<`,
//! `<=`, `>`, `>=`, `<=>`) desugar to at parse time.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The cmp interface is declared in
//! `handl-packages/cmp/cmp.hnl` via `op` declarations, and the Rust
//! comparison logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`CmpLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! The [`CmpModule`] re-export is preserved for backward compatibility.

use std::cmp::Ordering;
use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_utils::shared::{
    apply_fn, arr as to_arr, b as to_bool, find_extremum_by, num, require_array,
    require_function_and_array, require_one_arg,
};
use serde_json::Value as JsonValue;

/// The handl source declaring the cmp `op` interface — the single source
/// of truth for the cmp API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const CMP_HNL: &str = include_str!("../../../handl-packages/cmp/cmp.hnl");

/// The op names declared in `cmp.hnl`.
const CMP_OPS: &[&str] = &[
    "cmp.eq",
    "cmp.ne",
    "cmp.lt",
    "cmp.le",
    "cmp.gt",
    "cmp.ge",
    "cmp.compare",
    "cmp.min",
    "cmp.max",
    "cmp.clamp",
    "cmp.between",
    "cmp.in_range",
    "cmp.is_sorted",
    "cmp.is_sorted_desc",
    "cmp.sort",
    "cmp.sort_desc",
    "cmp.sort_by",
    "cmp.sort_by_desc",
    "cmp.min_by",
    "cmp.max_by",
];

// ---------------------------------------------------------------------------
// Comparison helpers (exposed for sibling crates)
// ---------------------------------------------------------------------------

fn value_kind(v: &JsonValue) -> u8 {
    match v {
        JsonValue::Null => 0,
        JsonValue::Bool(_) => 1,
        JsonValue::Number(_) => 2,
        JsonValue::String(_) => 3,
        JsonValue::Array(_) => 4,
        JsonValue::Object(_) => 5,
    }
}

/// Total ordering over arbitrary JSON values, used by every `cmp` function.
///
/// Ordering across kinds follows `value_kind` (null < bool < number < string
/// < array < object); within a kind the natural ordering is used. This is
/// exposed so sibling crates (e.g. a future `cmp-order`/`cmp-sort` split) can
/// reuse the same comparison semantics without re-implementing them.
pub fn compare_values(a: &JsonValue, b: &JsonValue) -> Ordering {
    value_kind(a)
        .cmp(&value_kind(b))
        .then_with(|| match (a, b) {
            (JsonValue::Null, JsonValue::Null) => Ordering::Equal,
            (JsonValue::Bool(aa), JsonValue::Bool(bb)) => aa.cmp(bb),
            (JsonValue::Number(aa), JsonValue::Number(bb)) => {
                aa.as_f64().unwrap().total_cmp(&bb.as_f64().unwrap())
            }
            (JsonValue::String(aa), JsonValue::String(bb)) => aa.cmp(bb),
            (JsonValue::Array(aa), JsonValue::Array(bb)) => {
                for (x, y) in aa.iter().zip(bb.iter()) {
                    let c = compare_values(x, y);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                aa.len().cmp(&bb.len())
            }
            (JsonValue::Object(aa), JsonValue::Object(bb)) => {
                if aa.len() != bb.len() {
                    return aa.len().cmp(&bb.len());
                }
                let mut av: Vec<_> = aa.iter().map(|(k, v)| (k.as_str(), v)).collect();
                let mut bv: Vec<_> = bb.iter().map(|(k, v)| (k.as_str(), v)).collect();
                av.sort_by(|x, y| x.0.cmp(y.0));
                bv.sort_by(|x, y| x.0.cmp(y.0));
                for ((ak, avv), (bk, bvv)) in av.iter().zip(bv.iter()) {
                    let c = ak.cmp(bk);
                    if c != Ordering::Equal {
                        return c;
                    }
                    let c = compare_values(avv, bvv);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            }
            _ => Ordering::Equal,
        })
}

fn compare_nfv(a: &serde_json::Value, b: &serde_json::Value) -> Ordering {
    compare_values(a, b)
}

fn require_two(
    args: &[serde_json::Value],
) -> Result<(&serde_json::Value, &serde_json::Value), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    Ok((&args[0], &args[1]))
}

fn require_three(
    args: &[serde_json::Value],
) -> Result<(&serde_json::Value, &serde_json::Value, &serde_json::Value), EvalError> {
    if args.len() < 3 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            3,
            args.len(),
        )));
    }
    Ok((&args[0], &args[1], &args[2]))
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Eq;
impl Function for Eq {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(a == b_arg))
    }
}

#[derive(Clone, Copy)]
pub struct Ne;
impl Function for Ne {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(a != b_arg))
    }
}

#[derive(Clone, Copy)]
pub struct Lt;
impl Function for Lt {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) == Ordering::Less))
    }
}

#[derive(Clone, Copy)]
pub struct Le;
impl Function for Le {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) != Ordering::Greater))
    }
}

#[derive(Clone, Copy)]
pub struct Gt;
impl Function for Gt {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) == Ordering::Greater))
    }
}

#[derive(Clone, Copy)]
pub struct Ge;
impl Function for Ge {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) != Ordering::Less))
    }
}

#[derive(Clone, Copy)]
pub struct Compare;
impl Function for Compare {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b_arg) = require_two(args)?;
        Ok(num(match compare_nfv(a, b_arg) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        }))
    }
}

#[derive(Clone, Copy)]
pub struct Min;
impl Function for Min {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let best = find_extremum_by(args, |a, b| compare_nfv(a, b) == Ordering::Less).ok_or(
            EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)),
        )?;
        Ok(best.clone())
    }
}

#[derive(Clone, Copy)]
pub struct Max;
impl Function for Max {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let best = find_extremum_by(args, |a, b| compare_nfv(a, b) == Ordering::Greater).ok_or(
            EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)),
        )?;
        Ok(best.clone())
    }
}

#[derive(Clone, Copy)]
pub struct Clamp;
impl Function for Clamp {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (value, lo, hi) = require_three(args)?;
        if compare_nfv(lo, hi) == Ordering::Greater {
            return Err(EvalError::Other("clamp bounds are inverted".to_string()));
        }
        if compare_nfv(value, lo) == Ordering::Less {
            return Ok(lo.clone());
        }
        if compare_nfv(value, hi) == Ordering::Greater {
            return Ok(hi.clone());
        }
        Ok(value.clone())
    }
}

fn between_impl(args: &[serde_json::Value]) -> Result<serde_json::Value, EvalError> {
    let (x, lo, hi) = require_three(args)?;
    Ok(to_bool(
        compare_nfv(lo, x) != Ordering::Greater && compare_nfv(x, hi) != Ordering::Greater,
    ))
}

#[derive(Clone, Copy)]
pub struct Between;
impl Function for Between {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        between_impl(args)
    }
}

#[derive(Clone, Copy)]
pub struct InRange;
impl Function for InRange {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        between_impl(args)
    }
}

fn is_sorted_impl(
    args: &[serde_json::Value],
    descending: bool,
) -> Result<serde_json::Value, EvalError> {
    let input = require_one_arg(args)?;
    let arr_val = require_array(input)?;
    for window in arr_val.windows(2) {
        let c = compare_values(&window[0], &window[1]);
        if descending && c == Ordering::Less {
            return Ok(to_bool(false));
        }
        if !descending && c == Ordering::Greater {
            return Ok(to_bool(false));
        }
    }
    Ok(to_bool(true))
}

#[derive(Clone, Copy)]
pub struct IsSorted;
impl Function for IsSorted {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        is_sorted_impl(args, false)
    }
}

#[derive(Clone, Copy)]
pub struct IsSortedDesc;
impl Function for IsSortedDesc {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        is_sorted_impl(args, true)
    }
}

fn sort_impl(args: &[serde_json::Value], descending: bool) -> Result<serde_json::Value, EvalError> {
    let input = require_one_arg(args)?;
    let arr_val = require_array(input)?;
    let mut sorted: Vec<JsonValue> = arr_val.to_vec();
    if descending {
        sorted.sort_by(|a, c| compare_values(c, a));
    } else {
        sorted.sort_by(|a, c| compare_values(a, c));
    }
    Ok(to_arr(&sorted))
}

#[derive(Clone, Copy)]
pub struct Sort;
impl Function for Sort {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        sort_impl(args, false)
    }
}

#[derive(Clone, Copy)]
pub struct SortDesc;
impl Function for SortDesc {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        sort_impl(args, true)
    }
}

// ── sort_by / sort_by_desc ─────────────────────────────────────────

fn sort_by_impl(
    args: &[serde_json::Value],
    ctx: &mut EvalContext,
    descending: bool,
) -> Result<serde_json::Value, EvalError> {
    let (fn_val, array) = require_function_and_array(args)?;
    let mut keyed: Vec<(f64, JsonValue)> = Vec::with_capacity(array.len());
    for item in array {
        let key = apply_fn(ctx, &fn_val, item)?;
        let num = key.as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        keyed.push((num, item.clone()));
    }
    if descending {
        keyed.sort_by(|a, c| c.0.total_cmp(&a.0));
    } else {
        keyed.sort_by(|a, c| a.0.total_cmp(&c.0));
    }
    let sorted: Vec<JsonValue> = keyed.into_iter().map(|(_, v)| v).collect();
    Ok(to_arr(&sorted))
}

#[derive(Clone, Copy)]
pub struct SortBy;
impl Function for SortBy {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        sort_by_impl(args, ctx, false)
    }
}

#[derive(Clone, Copy)]
pub struct SortByDesc;
impl Function for SortByDesc {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        sort_by_impl(args, ctx, true)
    }
}

// ── min_by / max_by ────────────────────────────────────────────────

fn extremum_by_impl(
    args: &[serde_json::Value],
    ctx: &mut EvalContext,
    pick_max: bool,
) -> Result<serde_json::Value, EvalError> {
    let (fn_val, array) = require_function_and_array(args)?;
    let mut best: Option<(f64, &JsonValue)> = None;
    for item in array {
        let key = apply_fn(ctx, &fn_val, item)?;
        let num = key.as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        match &best {
            None => best = Some((num, item)),
            Some((bn, _)) if pick_max && num > *bn => best = Some((num, item)),
            Some((bn, _)) if !pick_max && num < *bn => best = Some((num, item)),
            _ => {}
        }
    }
    match best {
        Some((_, v)) => Ok(serde_json::Value::from(v.clone())),
        None => Ok(serde_json::Value::from(JsonValue::Null)),
    }
}

#[derive(Clone, Copy)]
pub struct MinBy;
impl Function for MinBy {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        extremum_by_impl(args, ctx, false)
    }
}

#[derive(Clone, Copy)]
pub struct MaxBy;
impl Function for MaxBy {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        extremum_by_impl(args, ctx, true)
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for cmp, registering the Rust handls under
/// their op names.
pub fn cmp_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("cmp.eq", Arc::new(Eq));
    registry.register("cmp.ne", Arc::new(Ne));
    registry.register("cmp.lt", Arc::new(Lt));
    registry.register("cmp.le", Arc::new(Le));
    registry.register("cmp.gt", Arc::new(Gt));
    registry.register("cmp.ge", Arc::new(Ge));
    registry.register("cmp.compare", Arc::new(Compare));
    registry.register("cmp.min", Arc::new(Min));
    registry.register("cmp.max", Arc::new(Max));
    registry.register("cmp.clamp", Arc::new(Clamp));
    registry.register("cmp.between", Arc::new(Between));
    registry.register("cmp.in_range", Arc::new(InRange));
    registry.register("cmp.is_sorted", Arc::new(IsSorted));
    registry.register("cmp.is_sorted_desc", Arc::new(IsSortedDesc));
    registry.register("cmp.sort", Arc::new(Sort));
    registry.register("cmp.sort_desc", Arc::new(SortDesc));
    registry.register("cmp.sort_by", Arc::new(SortBy));
    registry.register("cmp.sort_by_desc", Arc::new(SortByDesc));
    registry.register("cmp.min_by", Arc::new(MinBy));
    registry.register("cmp.max_by", Arc::new(MaxBy));
    registry
}

// ---------------------------------------------------------------------------
// CmpModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `cmp` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// that register `CmpModule` directly.
#[derive(Clone, Copy)]
pub struct CmpModule;

impl Module for CmpModule {
    fn name(&self) -> &str {
        "cmp"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "eq" => Some(&Eq),
            "ne" => Some(&Ne),
            "lt" => Some(&Lt),
            "le" => Some(&Le),
            "gt" => Some(&Gt),
            "ge" => Some(&Ge),
            "compare" => Some(&Compare),
            "min" => Some(&Min),
            "max" => Some(&Max),
            "clamp" => Some(&Clamp),
            "between" => Some(&Between),
            "in_range" => Some(&InRange),
            "is_sorted" => Some(&IsSorted),
            "is_sorted_desc" => Some(&IsSortedDesc),
            "sort" => Some(&Sort),
            "sort_desc" => Some(&SortDesc),
            "sort_by" => Some(&SortBy),
            "sort_by_desc" => Some(&SortByDesc),
            "min_by" => Some(&MinBy),
            "max_by" => Some(&MaxBy),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// CmpLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `cmp` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The cmp interface is declared in `cmp.hnl` via `op`/`handl`
/// declarations; the Rust comparison handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct CmpLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl CmpLibraryPlugin {
    /// Create a new `CmpLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(cmp_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps("cmp", "lib.cmp", "lib-cmp", CMP_HNL, &[])
                .with_ops(CMP_OPS, registry),
        }
    }
}

impl Default for CmpLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for CmpLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for cmp with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`CmpLibraryPlugin`].
pub fn cmp_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(cmp_op_registry());
    HandlLibraryPlugin::new_with_caps("cmp", "lib.cmp", "lib-cmp", CMP_HNL, &[])
        .with_ops(CMP_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`CmpLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_cmp::LibraryPlugin`.
pub use CmpLibraryPlugin as LibraryPlugin;

#[cfg(test)]
mod tests;
