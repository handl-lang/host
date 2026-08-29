use std::cmp::Ordering;

use handl_plugin_vm::{Context, Function, GenericFunction};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;

use crate::shared::{
    apply_fn, arr as to_arr, b as to_bool, find_extremum_by, num, require_array,
    require_function_and_array, require_one_arg,
};

/// The `cmp` module — comparison and ordering utilities.
///
/// Provides functions callable as `cmp.eq(a, b)`, `cmp.lt(a, b)`,
/// `cmp.min(a, b, ...)`, etc. in handl expressions.
#[derive(Clone, Copy)]
pub struct CmpModule;

impl Module for CmpModule {
    fn name(&self) -> &str {
        "cmp"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
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

fn compare_values(a: &JsonValue, b: &JsonValue) -> Ordering {
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

fn compare_nfv(a: &NonFunctionValue, b: &NonFunctionValue) -> Ordering {
    compare_values(a.inner(), b.inner())
}

fn require_two(args: &[NonFunctionValue]) -> Result<(&NonFunctionValue, &NonFunctionValue), Error> {
    if args.len() < 2 {
        return Err(Error::ArgumentCount {
            expected: 2,
            got: args.len(),
        });
    }
    Ok((&args[0], &args[1]))
}

fn require_three(
    args: &[NonFunctionValue],
) -> Result<(&NonFunctionValue, &NonFunctionValue, &NonFunctionValue), Error> {
    if args.len() < 3 {
        return Err(Error::ArgumentCount {
            expected: 3,
            got: args.len(),
        });
    }
    Ok((&args[0], &args[1], &args[2]))
}

#[derive(Clone, Copy)]
pub struct Eq;
impl Function<NonFunctionValue> for Eq {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(a.inner() == b_arg.inner()))
    }
}

#[derive(Clone, Copy)]
pub struct Ne;
impl Function<NonFunctionValue> for Ne {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(a.inner() != b_arg.inner()))
    }
}

#[derive(Clone, Copy)]
pub struct Lt;
impl Function<NonFunctionValue> for Lt {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) == Ordering::Less))
    }
}

#[derive(Clone, Copy)]
pub struct Le;
impl Function<NonFunctionValue> for Le {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) != Ordering::Greater))
    }
}

#[derive(Clone, Copy)]
pub struct Gt;
impl Function<NonFunctionValue> for Gt {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) == Ordering::Greater))
    }
}

#[derive(Clone, Copy)]
pub struct Ge;
impl Function<NonFunctionValue> for Ge {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a, b_arg) = require_two(args)?;
        Ok(to_bool(compare_nfv(a, b_arg) != Ordering::Less))
    }
}

#[derive(Clone, Copy)]
pub struct Compare;
impl Function<NonFunctionValue> for Compare {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
impl Function<NonFunctionValue> for Min {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let best = find_extremum_by(args, |a, b| compare_nfv(a, b) == Ordering::Less).ok_or(
            Error::ArgumentCount {
                expected: 1,
                got: 0,
            },
        )?;
        Ok(best.clone())
    }
}

#[derive(Clone, Copy)]
pub struct Max;
impl Function<NonFunctionValue> for Max {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let best = find_extremum_by(args, |a, b| compare_nfv(a, b) == Ordering::Greater).ok_or(
            Error::ArgumentCount {
                expected: 1,
                got: 0,
            },
        )?;
        Ok(best.clone())
    }
}

#[derive(Clone, Copy)]
pub struct Clamp;
impl Function<NonFunctionValue> for Clamp {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (value, lo, hi) = require_three(args)?;
        if compare_nfv(lo, hi) == Ordering::Greater {
            return Err(Error::PrimitiveError(
                "clamp bounds are inverted".to_string(),
            ));
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

fn between_impl(args: &[NonFunctionValue]) -> Result<NonFunctionValue, Error> {
    let (x, lo, hi) = require_three(args)?;
    Ok(to_bool(
        compare_nfv(lo, x) != Ordering::Greater && compare_nfv(x, hi) != Ordering::Greater,
    ))
}

#[derive(Clone, Copy)]
pub struct Between;
impl Function<NonFunctionValue> for Between {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        between_impl(args)
    }
}

#[derive(Clone, Copy)]
pub struct InRange;
impl Function<NonFunctionValue> for InRange {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        between_impl(args)
    }
}

fn is_sorted_impl(args: &[NonFunctionValue], descending: bool) -> Result<NonFunctionValue, Error> {
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
impl Function<NonFunctionValue> for IsSorted {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        is_sorted_impl(args, false)
    }
}

#[derive(Clone, Copy)]
pub struct IsSortedDesc;
impl Function<NonFunctionValue> for IsSortedDesc {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        is_sorted_impl(args, true)
    }
}

fn sort_impl(args: &[NonFunctionValue], descending: bool) -> Result<NonFunctionValue, Error> {
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
impl Function<NonFunctionValue> for Sort {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        sort_impl(args, false)
    }
}

#[derive(Clone, Copy)]
pub struct SortDesc;
impl Function<NonFunctionValue> for SortDesc {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        sort_impl(args, true)
    }
}

// ── sort_by / sort_by_desc ─────────────────────────────────────────

fn sort_by_impl(
    args: &[NonFunctionValue],
    ctx: &mut dyn Context<NonFunctionValue>,
    descending: bool,
) -> Result<NonFunctionValue, Error> {
    let (fn_val, array) = require_function_and_array(args)?;
    let mut keyed: Vec<(f64, JsonValue)> = Vec::with_capacity(array.len());
    for item in array {
        let key = apply_fn(ctx, &fn_val, item)?;
        let num = key.as_number().ok_or(Error::TypeMismatch {
            expected: "number",
            got: "non-number",
        })?;
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
impl Function<NonFunctionValue> for SortBy {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        sort_by_impl(args, ctx, false)
    }
}

impl GenericFunction<NonFunctionValue> for SortBy {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

#[derive(Clone, Copy)]
pub struct SortByDesc;
impl Function<NonFunctionValue> for SortByDesc {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        sort_by_impl(args, ctx, true)
    }
}

impl GenericFunction<NonFunctionValue> for SortByDesc {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

// ── min_by / max_by ────────────────────────────────────────────────

fn extremum_by_impl(
    args: &[NonFunctionValue],
    ctx: &mut dyn Context<NonFunctionValue>,
    pick_max: bool,
) -> Result<NonFunctionValue, Error> {
    let (fn_val, array) = require_function_and_array(args)?;
    let mut best: Option<(f64, &JsonValue)> = None;
    for item in array {
        let key = apply_fn(ctx, &fn_val, item)?;
        let num = key.as_number().ok_or(Error::TypeMismatch {
            expected: "number",
            got: "non-number",
        })?;
        match &best {
            None => best = Some((num, item)),
            Some((bn, _)) if pick_max && num > *bn => best = Some((num, item)),
            Some((bn, _)) if !pick_max && num < *bn => best = Some((num, item)),
            _ => {}
        }
    }
    match best {
        Some((_, v)) => Ok(NonFunctionValue::new(v.clone())),
        None => Ok(NonFunctionValue::new(JsonValue::Null)),
    }
}

#[derive(Clone, Copy)]
pub struct MinBy;
impl Function<NonFunctionValue> for MinBy {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        extremum_by_impl(args, ctx, false)
    }
}

impl GenericFunction<NonFunctionValue> for MinBy {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

#[derive(Clone, Copy)]
pub struct MaxBy;
impl Function<NonFunctionValue> for MaxBy {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        extremum_by_impl(args, ctx, true)
    }
}

impl GenericFunction<NonFunctionValue> for MaxBy {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}
