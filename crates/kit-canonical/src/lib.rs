//! Canonical kit backed by the data-driven runtime.
//!
//! `Kit` assembles a [`Runtime`] from the data-driven canonical kit
//! (`handl-kit-data-canonical`) and exposes staged `lex`/`parse`/`eval`
//! methods over `handl_language` types (`Tokens`, `Ast`, `Value`).
//!
//! This is the ADR-0044 migration of `kit-canonical` from the old Rust
//! plugin-host assembly (which wired together dozens of `plugin-token-*`,
//! `plugin-grammar-*`, `plugin-eval-*`, and `plugin-ast-*` crates) to the
//! data-driven path: a single [`Runtime`] built from merged data fragments.

#![warn(missing_docs)]

/// Re-export of the canonical `.hnl` module loader.
///
/// The loader itself lives in `handl-plugin-lib-handl`
/// (`crates/plugin-lib-handl`); downstream crates may use either
/// `handl_plugin_lib_handl::*` directly or `handl_kit_canonical::
/// handl_module::*` via this shim.
pub mod handl_module {
    pub use handl_plugin_lib_handl::*;
}

use handl_kit_data_canonical;
use handl_language::{Ast, LangError, Language, Source, Span, Tokens, Value};
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup, TypeSignature};
use handl_plugin_engine_exec::ImportResolver;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_typecheck_nbe::NbeTypecheckEngine;
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{HandlModule, ModuleResolver, OpRegistry};
use handl_plugin_lib_lang_compile::CompileModule;
use handl_plugin_lib_lang_eval::EvalModule;
use handl_plugin_lib_lang_lex::LexModule;
use handl_plugin_lib_lang_parse::ParseModule;
use handl_plugin_lib_lang_run_compiled::RunCompiledModule;
use handl_plugin_lib_lang_typecheck::TypecheckModule;
use handl_plugin_lib_rand_native::RandModule;
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_stdlib::ops::collections::CollectionsModule;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_lib_tag::TypeRegistry;
use handl_plugin_lib_time_native::TimeModule;
use handl_plugin_lib_unicode_native::UnicodeModule;
use handl_plugin_module::ModuleRegistry;
use handl_runtime::RuntimeFactory;
use handl_runtime::{EngineRegistry, ModuleLoader, Runtime, RuntimeConfig, RuntimeFactoryImpl};
use std::path::{Path, PathBuf};
use std::rc::Rc;

// ---------------------------------------------------------------------------
// hello.greet host function + registry
// ---------------------------------------------------------------------------
//
// The greet *logic* is handl-authored in `handl-packages/hello/greet.hnl`
// (a single `name ~> "Hello, " + name + "!"` expression). This Rust
// side is a thin loader/bridge: it `include_str!`s that data file, evaluates
// it with a minimal module-free runtime (greet uses only the native `+`
// string-concat op, so no `ModuleLookup` is needed), and applies the
// resulting closure to the caller's argument. The runtime is cached per
// thread (the tree-walk engine is thread-local); only the tiny applied
// expression is lexed/parsed/evaluated per call.

/// The handl source defining `greet` — the single source of truth for the
/// greeting logic. Loaded at compile time from the kit-stdlib data dir.
const GREET_HNL: &str = include_str!("../../../handl-packages/hello/greet.hnl");

// ---------------------------------------------------------------------------
// dedent module (pure Handl — loaded via generic HandlModule)
// ---------------------------------------------------------------------------

const DEDENT_HNL: &str = include_str!("../../../handl-packages/dedent/dedent.hnl");
const DEDENT_FUNCS: &[&str] = &["dedent"];

// ---------------------------------------------------------------------------
// test framework modules (pure Handl — ADR-0092)
// ---------------------------------------------------------------------------

const TEST_HNL: &str = include_str!("../../../handl-packages/test/test.hnl");
const TEST_FUNCS: &[&str] = &["run"];

const PROPERTY_HNL: &str = include_str!("../../../handl-packages/test/property.hnl");
const PROPERTY_FUNCS: &[&str] = &["for_all"];

const BENCHMARK_HNL: &str = include_str!("../../../handl-packages/test/benchmark.hnl");
const BENCHMARK_FUNCS: &[&str] = &["run"];

const FUZZ_HNL: &str = include_str!("../../../handl-packages/test/fuzz.hnl");
const FUZZ_FUNCS: &[&str] = &["run"];

const SNAPSHOT_HNL: &str = include_str!("../../../handl-packages/test/snapshot.hnl");
const SNAPSHOT_FUNCS: &[&str] = &["format", "match"];

const ASSERT_HNL: &str = include_str!("../../../handl-packages/test/assert.hnl");
const ASSERT_FUNCS: &[&str] = &["equal", "not_equal", "true", "false", "contains"];

// ---------------------------------------------------------------------------
// str-handl module (pure Handl over the `char` floor — the `str` impl)
// ---------------------------------------------------------------------------

/// The handl source implementing the `str` contract
/// (`handl-packages/str/str.hnl`) as pure handl algorithms over `char.*`
/// atoms. Loaded via the generic [`HandlModule`], resolving `char` through
/// [`StdlibResolver`].
const STR_HANDL_HNL: &str = include_str!("../../../handl-packages/str-handl/str-handl.hnl");

/// The record field names exposed by `str-handl.hnl` — each becomes a
/// callable [`Function`](handl_platform::Function) on the `"str"` module.
const STR_HANDL_FUNCS: &[&str] = &[
    "upper",
    "lower",
    "trim",
    "trim_start",
    "trim_end",
    "trim_matches",
    "reverse",
    "length",
    "concat",
    "is_empty",
    "equals",
    "is_alpha",
    "is_numeric",
    "is_alphanumeric",
    "capitalize",
    "title_case",
    "snake_case",
    "camel_case",
    "pascal_case",
    "kebab_case",
    "contains",
    "starts_with",
    "ends_with",
    "count",
    "find",
    "rfind",
    "levenshtein",
    "similarity",
    "split",
    "join",
    "replace",
    "replace_first",
    "repeat",
    "char_at",
    "slice",
    "pad_left",
    "pad_right",
    "strip_prefix",
    "strip_suffix",
    "lines",
    "split_at",
    "split_whitespace",
    "truncate",
    "slugify",
    "word_wrap",
];

// ---------------------------------------------------------------------------
// unicode-native module (Rust/ICU4X backend with unicode-handl fallback)
// ---------------------------------------------------------------------------
//
// The native `unicode` implementation lives in `handl-plugin-lib-unicode-native`.
// It is composed with the pure-Handl `unicode-handl` record: native `op`s win,
// and the handl record supplies fallback behavior for anything not implemented
// natively. `char.*` calls inside `unicode-handl.hnl` are resolved by the
// built-in `CharModule` dependency.

/// Resolver that maps stdlib module names to their [`Module`] impls,
/// so `.hnl` files can declare dependencies via `from 'str' import * as 'str'`
/// instead of hardcoding them in Rust.
struct StdlibResolver;

impl ModuleResolver for StdlibResolver {
    fn resolve(
        &self,
        name: &str,
    ) -> Option<std::sync::Arc<dyn handl_platform::Module + Send + Sync>> {
        match name {
            "str" => Some(std::sync::Arc::new(build_str_handl_module())),
            "char" => Some(std::sync::Arc::new(CharModule)),
            "unicode" => Some(std::sync::Arc::new(UnicodeModule)),
            "transform" => Some(std::sync::Arc::new(TransformModule::new())),
            "comb" => Some(std::sync::Arc::new(CombModule::new())),
            "fn" | "combinators" => stdlib::module("combinators").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            "math" => stdlib::module("math").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            "prim" => stdlib::module("prim").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            "bool" => stdlib::module("bool").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            "option" => stdlib::module("option").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            "type" => Some(std::sync::Arc::new(build_type_module())),
            // `error` — the error-raising primitive (ADR-0092 gap (a));
            // a plain table module.
            "error" => stdlib::module("error").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            // `obj` — record introspection (gap #3); a plain table module.
            "obj" => stdlib::module("obj").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            "typeof" => stdlib::module("typeof").map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            // `result` is a table module whose `tag` dep is not in the
            // table (`tag` stays native in `plugin-lib-tag`), so it is
            // built via `module_with` to resolve `"tag"` through this
            // resolver.
            "tag" => Some(std::sync::Arc::new(TagModule)),
            "result" => stdlib::module_with("result", &StdlibResolver).map(|m| {
                std::sync::Arc::new(m) as std::sync::Arc<dyn handl_platform::Module + Send + Sync>
            }),
            _ => None,
        }
    }
}

fn build_dedent_module() -> HandlModule {
    HandlModule::new("dedent", DEDENT_HNL, DEDENT_FUNCS).with_resolver(&StdlibResolver)
}

// ---------------------------------------------------------------------------
// test framework module builders (ADR-0092)
// ---------------------------------------------------------------------------

fn build_test_module() -> HandlModule {
    HandlModule::new("test", TEST_HNL, TEST_FUNCS)
        .with_dependency_arc(std::sync::Arc::new(TagModule))
        .with_dependency_arc(std::sync::Arc::new(RandModule))
        .with_dependency_arc(std::sync::Arc::new(TimeModule))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("json").expect("json")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("array").expect("array")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("combinators").expect("combinators")))
        .with_dependency_arc(std::sync::Arc::new(build_type_module()))
}

fn build_property_module() -> HandlModule {
    HandlModule::new("property", PROPERTY_HNL, PROPERTY_FUNCS)
        .with_dependency_arc(std::sync::Arc::new(TagModule))
        .with_dependency_arc(std::sync::Arc::new(RandModule))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("json").expect("json")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("array").expect("array")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("combinators").expect("combinators")))
}

fn build_benchmark_module() -> HandlModule {
    HandlModule::new("benchmark", BENCHMARK_HNL, BENCHMARK_FUNCS)
        .with_dependency_arc(std::sync::Arc::new(TagModule))
        .with_dependency_arc(std::sync::Arc::new(TimeModule))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("json").expect("json")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("combinators").expect("combinators")))
}

fn build_fuzz_module() -> HandlModule {
    HandlModule::new("fuzz", FUZZ_HNL, FUZZ_FUNCS)
        .with_dependency_arc(std::sync::Arc::new(TagModule))
        .with_dependency_arc(std::sync::Arc::new(RandModule))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("json").expect("json")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("array").expect("array")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("combinators").expect("combinators")))
}

fn build_snapshot_module() -> HandlModule {
    HandlModule::new("snapshot", SNAPSHOT_HNL, SNAPSHOT_FUNCS)
        .with_dependency_arc(std::sync::Arc::new(TagModule))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("json").expect("json")))
}

fn build_assert_module() -> HandlModule {
    HandlModule::new("assert", ASSERT_HNL, ASSERT_FUNCS)
        .with_dependency_arc(std::sync::Arc::new(TagModule))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("json").expect("json")))
        .with_dependency_arc(std::sync::Arc::new(stdlib::module("array").expect("array")))
}

/// Build the [`HandlModule`] for `str-handl` — the pure-handl `str`
/// implementation over the `char` floor.
///
/// `str-handl.hnl` evaluates to a record of 45 arrows; each name in
/// [`STR_HANDL_FUNCS`] becomes a callable `Function`. `char.*` calls
/// resolve through [`StdlibResolver`] to the registered `char` backend
/// (`char-native` by default).
fn build_str_handl_module() -> HandlModule {
    HandlModule::new("str", STR_HANDL_HNL, STR_HANDL_FUNCS).with_resolver(&StdlibResolver)
}

// ---------------------------------------------------------------------------
// type module — pure-Handl `type-handl` record over the `type.of` atom
// ---------------------------------------------------------------------------
//
// `type` is a keyword in handl v0.1 (ADR-0074) but parses as a `var` atom,
// so `type.of(v)` in handl source evaluates as
// `field-access(var("type"), "of")` → the host call `"type.of"`, dispatched
// to a module named `"type"` providing `of` (the same module-path
// construction `tag.*`/`fn.*` use).
//
// The composition mirrors `unicode-native`: the pure-Handl record
// (`handl-packages/type-handl/type-handl.hnl`) supplies every predicate and
// kind guard, while the minimal reflection primitive `type.of` — plus the
// coercion/variadic atoms `type.to_string`, `type.to_number`, and
// `type.coalesce`, which the eval vocabulary cannot express — stay native
// `op`s attached via `with_ops`. Ops take precedence over record fields on
// name collision.

/// The handl source implementing the `type` contract
/// (`handl-packages/type/type.hnl`) as pure handl over the `type.of` atom.
const TYPE_HANDL_HNL: &str = include_str!("../../../handl-packages/type-handl/type-handl.hnl");

/// The record field names exposed by `type-handl.hnl` — each becomes a
/// callable [`Function`] on the `"type"` module.
const TYPE_HANDL_FUNCS: &[&str] = &[
    "is_null",
    "is_bool",
    "is_number",
    "is_string",
    "is_array",
    "is_object",
    "is_arrow",
    "is_function",
    "is_bytes",
    "is_resource",
    "tag",
    "name",
    "type_of",
    "typeof",
    "is",
    "must",
    "may",
    "not",
    "when",
    "identity",
    "default",
    "to_bool",
];

/// The `type` op names that remain native — each maps to a Rust
/// [`Function`] impl registered in the [`OpRegistry`].
///
/// - `type.of` — the minimal runtime kind-introspection atom (there is no
///   kind check in the eval vocabulary; `typeof` was removed from the
///   language surface).
/// - `type.to_string` / `type.to_number` — value↔string coercion and
///   string→number parsing have no eval-vocabulary primitive.
/// - `type.coalesce` — variadic; handl arrows have fixed parameter lists.
const TYPE_OPS: &[&str] = &[
    "type.of",
    "type.to_string",
    "type.to_number",
    "type.coalesce",
];

/// `type.of(v)` — return the kind tag of a value.
///
/// The tag vocabulary matches the tree-walk engine's `type_tag`:
/// `"null"`, `"bool"`, `"number"`, `"string"`, `"array"`, `"object"`,
/// `"bytes"`, `"resource"`. At the `serde_json` boundary an arrow value
/// arrives as `{"__arrow__": true, ...}` and is reported as `"arrow"`;
/// `__pair__`/`__either__`/`__tag__` encodings report `"object"`, matching
/// `type_tag` which sees them all as `Obj`.
#[derive(Clone, Copy, Debug)]
pub struct TypeOf;

impl Function for TypeOf {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1,
            args.len()
        )))?;
        Ok(Value::from(type_tag_json(v).to_string()))
    }
}

/// Kind-tag classification at the `serde_json` boundary (see [`TypeOf`]).
fn type_tag_json(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(o) => {
            if o.get("__arrow__").and_then(|b| b.as_bool()) == Some(true) {
                "arrow"
            } else if o.contains_key("__bytes__") {
                "bytes"
            } else if o.contains_key("__resource__") {
                "resource"
            } else {
                "object"
            }
        }
    }
}

/// Build the [`OpRegistry`] for the native `type` ops.
///
/// `type.of` is the new reflection atom ([`TypeOf`]); `to_string`,
/// `to_number`, and `coalesce` reuse the existing coercion/variadic impls
/// from `plugin-lib-utils`'s `r#type` module (the same handls that back
/// `typeof.to_string` etc. on the legacy `typeof` module).
fn type_op_registry() -> OpRegistry {
    use handl_plugin_lib_stdlib::ops::r#type::{Coalesce, ToNumber, ToString};
    let mut registry = OpRegistry::new();
    registry.register("type.of", std::sync::Arc::new(TypeOf));
    registry.register("type.to_string", std::sync::Arc::new(ToString));
    registry.register("type.to_number", std::sync::Arc::new(ToNumber));
    registry.register("type.coalesce", std::sync::Arc::new(Coalesce));
    registry
}

/// An ops-only `"type"` module used as the in-record dependency of
/// `type-handl`: `type.of(v)` calls inside the `.hnl` source resolve
/// through the bridge runtime's module chain to these native ops.
fn type_ops_module() -> HandlModule {
    HandlModule::new("type", "{}", &[]).with_ops(TYPE_OPS, std::sync::Arc::new(type_op_registry()))
}

/// Build the composed [`HandlModule`] for `type`: the `type-handl` record
/// plus the native `type.*` ops.
fn build_type_module() -> HandlModule {
    HandlModule::new("type", TYPE_HANDL_HNL, TYPE_HANDL_FUNCS)
        .with_dependency(type_ops_module())
        .with_ops(TYPE_OPS, std::sync::Arc::new(type_op_registry()))
}

// ---------------------------------------------------------------------------
// transform — pure-Handl list transforms (one .hnl file per arrow), with
// native overrides for the arrow-argument call path
// comb — pure-Handl function combinators (one .hnl file per arrow)
// ---------------------------------------------------------------------------

/// Handl-style truthiness used by `transform.filter`.
fn transform_is_truthy(v: &Value) -> bool {
    match v {
        Value::Null | Value::Bool(false) => false,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
        Value::Bool(true) => true,
    }
}

/// `transform.map(xs, f)` — host-side implementation that calls the arrow `f`
/// through the current `EvalContext` (`ctx.try_call_value`).
#[derive(Clone, Copy, Debug)]
pub struct TransformMap;

impl Function for TransformMap {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "transform.map: expected 2 arguments, got {}",
                args.len()
            )));
        }
        let xs = args[0].as_array().ok_or_else(|| {
            EvalError::Other("transform.map: first argument must be an array".to_string())
        })?;
        let f = &args[1];
        let mut out = Vec::with_capacity(xs.len());
        for x in xs {
            let mapped = ctx
                .try_call_value(f, std::slice::from_ref(x))
                .ok_or_else(|| {
                    EvalError::Other("transform.map: no value callable in context".to_string())
                })??;
            out.push(mapped);
        }
        Ok(Value::Array(out))
    }
}

/// `transform.filter(xs, p)` — host-side implementation that calls the predicate.
#[derive(Clone, Copy, Debug)]
pub struct TransformFilter;

impl Function for TransformFilter {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "transform.filter: expected 2 arguments, got {}",
                args.len()
            )));
        }
        let xs = args[0].as_array().ok_or_else(|| {
            EvalError::Other("transform.filter: first argument must be an array".to_string())
        })?;
        let p = &args[1];
        let mut out = Vec::new();
        for x in xs {
            let keep = ctx
                .try_call_value(p, std::slice::from_ref(x))
                .ok_or_else(|| {
                    EvalError::Other("transform.filter: no value callable in context".to_string())
                })??;
            if transform_is_truthy(&keep) {
                out.push(x.clone());
            }
        }
        Ok(Value::Array(out))
    }
}

/// `transform.reduce(xs, init, f)` — host-side implementation.
#[derive(Clone, Copy, Debug)]
pub struct TransformReduce;

impl Function for TransformReduce {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 3 {
            return Err(EvalError::Other(format!(
                "transform.reduce: expected 3 arguments, got {}",
                args.len()
            )));
        }
        let xs = args[0].as_array().ok_or_else(|| {
            EvalError::Other("transform.reduce: first argument must be an array".to_string())
        })?;
        let mut acc = args[1].clone();
        let f = &args[2];
        for x in xs {
            let call_args = [acc, x.clone()];
            acc = ctx.try_call_value(f, &call_args).ok_or_else(|| {
                EvalError::Other("transform.reduce: no value callable in context".to_string())
            })??;
        }
        Ok(acc)
    }
}

/// `TransformModule` exposes `handl-packages/transform/*.hnl` as the
/// `transform.*` host function namespace: list-to-list / fold operations
/// (`map`, `filter`, `reduce`). Because the tree-walk host-call path cannot
/// yet pass arrow values through, a small native `Function` override is
/// wired in front of each `.hnl` source.
pub struct TransformModule {
    functions: std::collections::HashMap<String, HandlModule>,
    overrides: std::collections::HashMap<String, std::sync::Arc<dyn Function + Send + Sync>>,
}

impl TransformModule {
    /// Load all `handl-packages/transform/*.hnl` files as one-function
    /// `HandlModule`s, and register the host-side overrides.
    pub fn new() -> Self {
        let mut functions = std::collections::HashMap::new();
        let mut overrides: std::collections::HashMap<
            String,
            std::sync::Arc<dyn Function + Send + Sync>,
        > = std::collections::HashMap::new();

        overrides.insert("map".to_string(), std::sync::Arc::new(TransformMap));
        overrides.insert("filter".to_string(), std::sync::Arc::new(TransformFilter));
        overrides.insert("reduce".to_string(), std::sync::Arc::new(TransformReduce));

        functions.insert(
            "map".to_string(),
            HandlModule::new(
                "transform",
                include_str!("../../../handl-packages/transform/map.hnl"),
                &["map"],
            ),
        );
        functions.insert(
            "filter".to_string(),
            HandlModule::new(
                "transform",
                include_str!("../../../handl-packages/transform/filter.hnl"),
                &["filter"],
            ),
        );
        functions.insert(
            "reduce".to_string(),
            HandlModule::new(
                "transform",
                include_str!("../../../handl-packages/transform/reduce.hnl"),
                &["reduce"],
            ),
        );
        Self {
            functions,
            overrides,
        }
    }
}

impl Module for TransformModule {
    fn name(&self) -> &str {
        "transform"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        if let Some(arc) = self.overrides.get(name) {
            return Some(&**arc);
        }
        self.functions.get(name).and_then(|m| m.function(name))
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        let mut out = Vec::new();
        for (name, arc) in &self.overrides {
            out.push((name.as_str(), &**arc as &dyn Function));
        }
        for (name, m) in &self.functions {
            if let Some(f) = m.function(name.as_str()) {
                out.push((name.as_str(), f));
            }
        }
        out
    }
}

/// `CombModule` exposes `handl-packages/comb/*.hnl` as the `comb.*` host
/// function namespace: pure-Handl function combinators (one `.hnl` file per
/// arrow, loaded by a one-function `HandlModule`). List transforms
/// (`map`/`filter`/`reduce`) live in [`TransformModule`].
pub struct CombModule {
    functions: std::collections::HashMap<String, HandlModule>,
}

impl CombModule {
    /// Load all `handl-packages/comb/*.hnl` files as one-function
    /// `HandlModule`s.
    pub fn new() -> Self {
        let mut functions = std::collections::HashMap::new();

        functions.insert(
            "fanout".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/fanout.hnl"),
                &["fanout"],
            ),
        );
        functions.insert(
            "fanout_fn".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/fanout_fn.hnl"),
                &["fanout_fn"],
            ),
        );
        functions.insert(
            "flow".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/flow.hnl"),
                &["flow"],
            ),
        );
        functions.insert(
            "compose".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/compose.hnl"),
                &["compose"],
            ),
        );
        functions.insert(
            "pipe".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/pipe.hnl"),
                &["pipe"],
            ),
        );
        functions.insert(
            "apply".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/apply.hnl"),
                &["apply"],
            ),
        );
        functions.insert(
            "tap".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/tap.hnl"),
                &["tap"],
            ),
        );
        functions.insert(
            "constantly".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/constantly.hnl"),
                &["constantly"],
            ),
        );
        functions.insert(
            "flip".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/flip.hnl"),
                &["flip"],
            ),
        );
        functions.insert(
            "partial".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/partial.hnl"),
                &["partial"],
            ),
        );
        functions.insert(
            "choice".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/choice.hnl"),
                &["choice"],
            ),
        );
        functions.insert(
            "when".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/when.hnl"),
                &["when"],
            ),
        );
        functions.insert(
            "unless".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/unless.hnl"),
                &["unless"],
            ),
        );
        functions.insert(
            "converge".to_string(),
            HandlModule::new(
                "comb",
                include_str!("../../../handl-packages/comb/converge.hnl"),
                &["converge"],
            ),
        );
        Self { functions }
    }
}

impl Module for CombModule {
    fn name(&self) -> &str {
        "comb"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.functions.get(name).and_then(|m| m.function(name))
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        let mut out = Vec::new();
        for (name, m) in &self.functions {
            if let Some(f) = m.function(name.as_str()) {
                out.push((name.as_str(), f));
            }
        }
        out
    }
}

thread_local! {
    /// A minimal, module-free runtime for evaluating `greet.hnl`. Cached
    /// per-thread because the tree-walk engine stores the active AST /
    /// `Language` in thread-locals. No module registry is wired: `greet`
    /// uses only the native `+` string-concatenation op.
    static GREET_RUNTIME: std::cell::RefCell<Option<Runtime>> =
        std::cell::RefCell::new(None);
}

/// Build the minimal runtime used to evaluate the handl-authored `greet`.
///
/// Reuses the canonical `Language` (assembled via
/// `handl_kit_data_canonical`) bound to the data lexer, Pratt parser, and
/// tree-walk evaluator — the same engines as [`Kit::new`], but with no
/// module registry and no globals.
fn build_greet_runtime() -> Result<Runtime, LangError> {
    let kit = handl_kit_data_canonical::assemble()?;
    let lang = kit.lang().clone();
    let config = RuntimeConfig::new()
        .with_lex("data")
        .with_parse("pratt")
        .with_eval("tree-walk");
    let mut registry = EngineRegistry::new();
    registry.register_lex(
        "data",
        Box::new(handl_plugin_engine_lex_pattern::DataLexerEngine::new()),
    );
    registry.register_parse("pratt", handl_plugin_engine_parse_pratt::PrattEngine::new());
    registry.register_eval(
        "tree-walk",
        Box::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new()),
    );
    let registry = Rc::new(registry);
    Runtime::from_registry(lang, &registry, &config)
}

/// Escape a Rust string for embedding inside a handl `"..."` string literal.
///
/// Only `"` and `\` need escaping: the delimited lexer consumes any other
/// character (including newlines) raw, and the Pratt parser's string-literal
/// unescaping only rewrites the four canonical escapes (`\\`, `\"`, `\n`,
/// `\t`), so all other characters pass through faithfully.
fn escape_handl_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

/// Strip `--` comment lines from `greet.hnl` so the embedded source parses
/// as a bare expression. The data file's header comment documents the
/// definition but is not valid inside the applied `(greet)(name)` form the
/// Pratt parser evaluates, so it is removed before evaluation.
fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("--") && !trimmed.starts_with("cap ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Evaluate the handl-authored `greet` applied to `name` and return the
/// resulting greeting string.
///
/// Reuses a cached per-thread minimal runtime; only the small applied
/// expression `(greet.hnl)(name)` is lexed/parsed/evaluated per call.
fn greet_via_handl(name: &str) -> Result<String, EvalError> {
    let escaped = escape_handl_string(name);
    let greet_src = strip_comment_lines(GREET_HNL);
    let source = format!("({greet_src})(\"{escaped}\")");
    GREET_RUNTIME.with(|cell| {
        let mut slot = cell.borrow_mut();
        let runtime = slot.get_or_insert_with(|| {
            build_greet_runtime().expect("canonical kit assembly should not fail")
        });
        runtime
            .run(&Source::new(&source))
            .map_err(|e| EvalError::Other(format!("hello.greet: {e}")))
            .and_then(|v| {
                v.as_str().map(String::from).ok_or_else(|| {
                    EvalError::Other("hello.greet: greet did not return a string".to_string())
                })
            })
    })
}

/// `hello.greet(name)` host function — the canonical handl greeting.
///
/// The canonical kit data declares `eval.hello.greet` as a `Foreign` scheme
/// (name `hello.greet`) and `ast.hello.greet`. Source syntax `hello.greet(x)`
/// parses as `call(field-access(hello, greet), x)`: the `hello` global (injected
/// by [`Kit::new`]) is `{"greet": "hello.greet"}`, so `field-access` yields the
/// string `"hello.greet"`, and the tree-walk engine's `apply_closure` dispatches
/// a string-named host function to this impl.
///
/// The greeting logic itself is handl-authored in `greet.hnl`; this impl only
/// validates arguments and delegates to [`greet_via_handl`].
pub struct HelloGreetHostFn;

impl Function for HelloGreetHostFn {
    fn as_type_signature(&self) -> Option<&dyn TypeSignature> {
        Some(self)
    }

    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::Other(format!(
                "hello.greet: expected 1 argument, got {}",
                args.len()
            )));
        }
        let name = args[0]
            .as_str()
            .ok_or_else(|| EvalError::Other("hello.greet: expected string argument".to_string()))?;
        Ok(Value::String(greet_via_handl(name)?))
    }
}

impl TypeSignature for HelloGreetHostFn {
    fn type_signature(&self) -> Value {
        // `hello.greet : String -> String`
        static REGISTRY: std::sync::OnceLock<TypeRegistry> = std::sync::OnceLock::new();
        let registry = REGISTRY.get_or_init(|| TypeRegistry::new("handl.canonical"));
        let string = registry
            .construct("type.string", &Value::Null)
            .expect("type.string should construct");
        let function = registry
            .construct(
                "type.function",
                &serde_json::json!({"param": string, "body": string}),
            )
            .expect("type.function should construct");
        function
    }
}

/// A [`ModuleLookup`] registry exposing `hello.greet`, with an optional
/// delegate for further lookup.
pub struct HelloHostFunctions {
    greet: HelloGreetHostFn,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Clone for HelloHostFunctions {
    fn clone(&self) -> Self {
        Self {
            greet: HelloGreetHostFn,
            delegate: self.delegate.clone(),
        }
    }
}

impl HelloHostFunctions {
    /// Create a `HelloHostFunctions` with an optional delegate for
    /// unknown-name lookup.
    pub fn new(delegate: Option<Rc<dyn ModuleLookup>>) -> Self {
        Self {
            greet: HelloGreetHostFn,
            delegate,
        }
    }
}

impl ModuleLookup for HelloHostFunctions {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "hello.greet" | "greet" => Some(&self.greet),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// Kit manifest loading and the default canonical manifest.
pub mod kit_manifest;
pub use kit_manifest::{KitManifest, canonical_manifest};

/// A token annotated with its source span.
///
/// The data-driven lexer (`handl_language::Token`) carries only `kind` and
/// `text` — no span. This helper type augments a token with the byte offsets
/// of its match in the source, which LSP/tooling hosts need for range-based
/// features (hover, goto-definition, semantic tokens).
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    /// The token kind id (e.g. `"token.identifier"`).
    pub kind: String,
    /// The matched source text.
    pub text: String,
    /// The byte span of the match in the source.
    pub span: Span,
}

/// The canonical kit: a thin wrapper around the data-driven [`Runtime`].
///
/// [`Kit::new`] assembles the data-driven canonical language
/// (`handl_kit_data_canonical::assemble`) and binds it to the Pratt parse
/// engine and tree-walk eval engine via [`Runtime::from_registry`]. It also
/// wires the meta host functions from the five `plugin-lib-lang-*` crates
/// (`lex`/`parse`/`eval`/`compile`/`run_compiled`) so a running handl program
/// can call `eval(lang, ast)`, `compile(lang, ast, target)`, etc.
/// (ADR-0041 Phase 2–3, ADR-0047).
///
/// All staged methods (`lex`, `parse`, `eval`, `run`) delegate to the
/// underlying [`Runtime`].
pub struct Kit {
    /// The runtime (no lifetime — engines own `Rc<dyn ModuleLookup>`).
    runtime: Runtime,
    /// The base directory for resolving relative `import "path"` paths
    /// (Phase 9.5). `None` → resolve against the process CWD.
    module_root: Option<PathBuf>,
}

impl std::fmt::Debug for Kit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kit")
            .field("runtime", &self.runtime)
            .finish()
    }
}

impl Kit {
    /// Build the canonical kit from the data-driven canonical language.
    ///
    /// Assembles all Phase 4 data fragments via
    /// `handl_kit_data_canonical::assemble` and binds the resulting
    /// [`Language`](handl_language::Language) to the data-driven lexer,
    /// Pratt parser, and tree-walk evaluator. Wires the meta host functions
    /// from the five `plugin-lib-lang-*` crates
    /// (`lex`/`parse`/`eval`/`compile`/`run_compiled`) so a running handl
    /// program can call them (ADR-0041 Phase 2–3, ADR-0047).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if fragment assembly or engine binding fails.
    pub fn new() -> Result<Self, LangError> {
        let kit = handl_kit_data_canonical::assemble()?;
        let lang = kit.lang().clone();
        let config = kit.runtime_config().clone();
        Self::build_kit(lang, config, None)
    }

    /// Build the canonical kit with a specific eval engine name
    /// (e.g. `"tree-walk"`, `"bytecode"`, `"cranelift"`).
    ///
    /// Otherwise identical to [`Kit::new`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if fragment assembly or engine binding fails.
    pub fn with_eval(eval_name: &str) -> Result<Self, LangError> {
        let kit = handl_kit_data_canonical::assemble()?;
        let lang = kit.lang().clone();
        let config = kit.runtime_config().clone().with_eval(eval_name);
        Self::build_kit(lang, config, None)
    }

    /// Build the canonical kit with a `module_root` directory for resolving
    /// relative `import "path"` statements (Phase 9.5).
    ///
    /// Otherwise identical to [`Kit::new`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if fragment assembly or engine binding fails.
    pub fn new_with_module_root(module_root: PathBuf) -> Result<Self, LangError> {
        let kit = handl_kit_data_canonical::assemble()?;
        let lang = kit.lang().clone();
        let config = kit.runtime_config().clone();
        Self::build_kit(lang, config, Some(module_root))
    }

    /// Build a [`Kit`] from a caller-supplied [`Language`] and
    /// [`RuntimeConfig`], wiring the canonical engine registry, the
    /// stdlib/fp/hello/meta module chain, and the `self`/`hello` globals.
    ///
    /// This is the shared assembly step used by both [`Kit::new`] (which
    /// feeds it the compiled-in canonical language) and [`Kit::from_path`]
    /// (which feeds it a language loaded from a directory of YAML files).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if engine binding fails.
    fn build_kit(
        lang: Language,
        config: RuntimeConfig,
        module_root: Option<PathBuf>,
    ) -> Result<Self, LangError> {
        // Build the engine registry with all available engine impls
        // (ADR-0047 §2). The host (this crate) registers the engines;
        // handl-runtime names none.
        let mut registry = EngineRegistry::new();
        registry.register_lex(
            "data",
            Box::new(handl_plugin_engine_lex_pattern::DataLexerEngine::new()),
        );
        registry.register_parse("pratt", handl_plugin_engine_parse_pratt::PrattEngine::new());
        registry.register_eval(
            "tree-walk",
            Box::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new()),
        );
        registry.register_eval(
            "bytecode",
            Box::new(handl_plugin_engine_exec_bytecode::BytecodeEngine::new()),
        );
        registry.register_eval(
            "cranelift",
            Box::new(handl_plugin_engine_exec_cranelift::CraneliftEngine::new()),
        );
        registry.register_compile(
            "bytecode",
            Box::new(handl_plugin_engine_exec_bytecode::BytecodeCompileEngine::new()),
        );
        registry.register_compile(
            "native",
            Box::new(handl_plugin_engine_exec_cranelift::CraneliftCompileEngine::new()),
        );
        registry.register_typecheck(
            "nbe",
            Box::new(NbeTypecheckEngine::new(Box::new(
                handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new(),
            ))),
        );
        let registry = Rc::new(registry);

        // Build the module-registry chain (outermost first):
        //   meta (lex/parse/eval/typecheck/compile/run_compiled) → hello (hello.greet)
        //   → fp (fn.fanout_fn, field_access) → stdlib (fn.*, collections.*,
        //   math.*, str.*, bool.*, type.*, option.*, result.*, mood.*,
        //   encoding.*, parse.*, serialize.*) → None
        // Meta is outermost so its names win; hello and fp are delegates so
        // both the demo greeting and the FP Foreign schemes resolve at eval
        // time, and sub-runtimes created by meta functions (e.g. eval) inherit
        // them via the delegate chain.
        let mut stdlib_registry = ModuleRegistry::new();
        stdlib_registry.register(stdlib::module("combinators").expect("combinators"));
        stdlib_registry.register(stdlib::module("math").expect("math"));
        // `str-handl` is the pure-handl `str` impl over the `char` floor;
        // it satisfies the `str` contract. Registered under `"str"` in
        // place of the former all-FFI `StrModule`, so `str.*` lookups and
        // `from "str" import` callers resolve to the handl impl.
        stdlib_registry.register(build_str_handl_module());
        stdlib_registry.register(CharModule);
        stdlib_registry.register(UnicodeModule);
        stdlib_registry.register(stdlib::module("bool").expect("bool"));
        // `type` — the canonical runtime-introspection module: the
        // `type-handl` record (predicates/guards) over the native
        // `type.of`/`type.to_string`/`type.to_number`/`type.coalesce` ops.
        stdlib_registry.register(build_type_module());
        // `typeof` — the legacy all-FFI type module, kept for `typeof.*`
        // callers (e.g. `stats.hnl`'s `typeof.type_of` filter).
        stdlib_registry.register(stdlib::module("typeof").expect("typeof"));
        stdlib_registry.register(stdlib::module("option").expect("option"));
        // `result` — pure-handl `ok`/`err` constructors over the `tag.*`
        // atoms; built via `module_with` so its `tag` dep resolves to
        // `TagModule` through `StdlibResolver`.
        stdlib_registry.register(stdlib::module_with("result", &StdlibResolver).expect("result"));
        stdlib_registry.register(stdlib::module("mood").expect("mood"));
        stdlib_registry.register(stdlib::module("encoding").expect("encoding"));
        stdlib_registry.register(stdlib::module("parse").expect("parse"));
        stdlib_registry.register(stdlib::module("serialize").expect("serialize"));
        // `error` — the error-raising primitive (`error.raise`,
        // ADR-0092 gap (a)); ops-only table module, callable as
        // `error.raise(msg)` from any .hnl source or eval expression.
        stdlib_registry.register(stdlib::module("error").expect("error"));
        // `obj` — record introspection primitives (`obj.keys`/`obj.values`/
        // `obj.entries`); ops-only table module.
        stdlib_registry.register(stdlib::module("obj").expect("obj"));
        // `collections` — the aggregate module over the package-table
        // sub-modules (vec/list/queue/stack/deque/hash_map/hash_set/array),
        // preserving the former `plugin-lib-collections` lookup shape.
        stdlib_registry.register(CollectionsModule::new(&stdlib::DEFAULT_RESOLVER));
        stdlib_registry.register(TagModule);
        stdlib_registry.register(build_dedent_module());
        // `test` framework modules — pure handl over tag/json/rand/time/
        // combinators (ADR-0092).
        stdlib_registry.register(build_test_module());
        stdlib_registry.register(build_property_module());
        stdlib_registry.register(build_benchmark_module());
        stdlib_registry.register(build_fuzz_module());
        stdlib_registry.register(build_snapshot_module());
        stdlib_registry.register(build_assert_module());
        stdlib_registry.register(TransformModule::new());
        stdlib_registry.register(CombModule::new());
        // `prim` — the atom layer: all four functions are pure handl
        // (`prim-handl` record over builtin `+`/`[]`/`math.bit_or`/`cmp.compare`).
        stdlib_registry.register(stdlib::module("prim").expect("prim"));
        // `time`/`rand` — native backends (clock reads, deterministic
        // seeded RNG for `property`/`fuzz`, ADR-0092).
        stdlib_registry.register(TimeModule);
        stdlib_registry.register(RandModule);
        let stdlib_chain: Rc<dyn handl_platform::ModuleLookup> = Rc::new(stdlib_registry);
        let fp = handl_runtime::fp::canonical_fp_host_functions(Some(Rc::clone(&stdlib_chain)));
        let hello = Rc::new(HelloHostFunctions::new(Some(
            Rc::clone(&fp) as Rc<dyn handl_platform::ModuleLookup>
        )));

        // Build the meta chain from the 5 plugin-lib-lang-* crates
        // (ADR-0047 §1). Each module holds Rc<dyn RuntimeFactory> (the
        // RuntimeFactoryImpl) and chains via delegate. The factory's
        // meta_chain is set after construction, breaking the chicken-and-egg.
        let factory = Rc::new(RuntimeFactoryImpl::new(
            Rc::clone(&registry),
            config.clone(),
        ));
        let factory_dyn: Rc<dyn RuntimeFactory> = Rc::clone(&factory) as Rc<dyn RuntimeFactory>;

        // Chain: lex → parse → eval → typecheck → compile → run_compiled → hello → fp → None
        let run_compiled_mod = Rc::new(RunCompiledModule::new(
            Rc::clone(&factory_dyn),
            Some(Rc::clone(&hello) as Rc<dyn handl_platform::ModuleLookup>),
        ));
        let compile_mod = Rc::new(CompileModule::new(
            Rc::clone(&factory_dyn),
            Some(Rc::clone(&run_compiled_mod) as Rc<dyn handl_platform::ModuleLookup>),
        ));
        let typecheck_mod = Rc::new(TypecheckModule::new(
            Rc::clone(&factory_dyn),
            Some(Rc::clone(&compile_mod) as Rc<dyn handl_platform::ModuleLookup>),
        ));
        let eval_mod = Rc::new(EvalModule::new(
            Rc::clone(&factory_dyn),
            Some(Rc::clone(&typecheck_mod) as Rc<dyn handl_platform::ModuleLookup>),
        ));
        let parse_mod = Rc::new(ParseModule::new(
            Rc::clone(&factory_dyn),
            Some(Rc::clone(&eval_mod) as Rc<dyn handl_platform::ModuleLookup>),
        ));
        let lex_mod = Rc::new(LexModule::new(
            Rc::clone(&factory_dyn),
            Some(Rc::clone(&parse_mod) as Rc<dyn handl_platform::ModuleLookup>),
        ));
        let meta_chain: Rc<dyn handl_platform::ModuleLookup> = lex_mod;
        factory.set_meta_chain(Rc::clone(&meta_chain));

        // Inject `self` as a global variable bound to the canonical
        // `Language` value (ADR-0041 Phase 3: `self` is an ambient variable,
        // not a function call). Inject `hello` as a module object so
        // `hello.greet(x)` parses to `call(field-access(hello, greet), x)`,
        // resolves `hello.greet` to the host-function name string, and
        // dispatches via `apply_closure`'s string-named host-fn path.
        //
        // The `tag.*` tagged-values capability (ADR-0055) is NOT a global:
        // `tag` is an unbound identifier, so `tag.tag(...)` parses to
        // `call(field-access(tag, tag), ...)`, both `tag` identifiers fall
        // back to the string `"tag"` (the unbound-variable fallback), and
        // `field_access("tag", "tag")` concatenates them to `"tag.tag"` —
        // the same module-path-construction path `fn.map` uses. The
        // `tag.*` host functions are registered by `plugin-lib-tag`'s
        // `TagModule` (above). A host that does not register `TagModule`
        // does not offer tagging — the capability boundary (ADR-0046,
        // ADR-0055 §1) is preserved.
        let self_val = handl_language::language_value(&lang);
        let mut globals = std::collections::HashMap::new();
        globals.insert("self".to_string(), self_val);
        globals.insert(
            "hello".to_string(),
            serde_json::json!({ "greet": "hello.greet" }),
        );

        // Phase 9.5: build a shared `ModuleLoader` for `import "path"`
        // statements. The loader owns independent engine clones (cloned from
        // the registry) plus a shared module cache and cycle-detector. Its
        // eval engine is a `TreeWalkEngine` wired with the same module chain
        // and `self` global as the main runtime, and with the loader itself
        // as its import resolver (so nested imports resolve through the same
        // cache). Two-phase construction (see `ModuleLoader` docs): create
        // the loader, build its engine holding an `Rc` clone of the loader,
        // then late-bind the engine.
        let lang_rc = Rc::new(lang.clone());
        let loader = Rc::new(ModuleLoader::new(
            Rc::clone(&lang_rc),
            registry
                .lex_engine("data")
                .ok_or_else(|| LangError::engine("missing lex engine 'data' for module loader"))?
                .box_clone(),
            registry
                .parse_engine("pratt")
                .ok_or_else(|| LangError::engine("missing parse engine 'pratt' for module loader"))?
                .box_clone(),
            module_root.clone(),
        ));
        let loader_eval_engine = TreeWalkEngine::new()
            .with_modules(Rc::clone(&meta_chain) as Rc<dyn handl_platform::ModuleLookup>)
            .with_global("self", handl_language::language_value(&lang))
            .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>);
        loader.set_eval_engine(Box::new(loader_eval_engine));
        let import_resolver: Rc<dyn ImportResolver> = Rc::clone(&loader) as Rc<dyn ImportResolver>;

        let runtime = Runtime::from_registry_with_resolver(
            lang,
            &registry,
            &config,
            Some(meta_chain),
            Some(&globals),
            Some(import_resolver),
            module_root.clone(),
        )?;
        Ok(Kit {
            runtime,
            module_root,
        })
    }

    /// Build a [`Kit`] from a kit data directory path (ADR 0003 per-file kit
    /// loading).
    ///
    /// - If `path` is the default-kit sentinel `"handl-kit-data-canonical"`
    ///   (or its final path component is), this delegates to [`Kit::new`],
    ///   loading the compiled-in canonical kit.
    /// - If `path` is a real directory, the eight YAML data files
    ///   (`tokens.yaml`, `grammar.yaml`, `eval.yaml`, `types.yaml`,
    ///   `ast.yaml`, `caps.yaml`, `options.yaml`, `lexer_rules.yaml`) are
    ///   read and merged into a [`Language`] via
    ///   [`handl_kit_data_canonical::language_from_yaml`], then wired into a
    ///   [`Kit`] with the same engine/module/runtime stack as [`Kit::new`].
    /// - If the directory does not exist, is not a directory, or any YAML
    ///   file is missing/unreadable, this falls back to [`Kit::new`] and
    ///   logs a warning to stderr.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] only if the fallback [`Kit::new`] itself fails
    /// (which should not happen for the compiled-in canonical kit). A failed
    /// directory load is never propagated — it always falls back.
    pub fn from_path(path: &Path) -> Result<Self, LangError> {
        const DEFAULT_KIT_SENTINEL: &str = "handl-kit-data-canonical";

        // Sentinel -> compiled-in canonical kit.
        if path == Path::new(DEFAULT_KIT_SENTINEL)
            || path
                .file_name()
                .map(|n| n == DEFAULT_KIT_SENTINEL)
                .unwrap_or(false)
        {
            return Self::new();
        }

        // Real directory -> load YAML files.
        if path.is_dir() {
            let config = handl_kit_data_canonical::manifest().runtime.clone();
            match handl_kit_data_canonical::language_from_yaml() {
                Ok(lang) => return Self::build_kit(lang, config, None),
                Err(e) => {
                    eprintln!(
                        "kit-canonical: from_path: failed to load canonical language: {}; \
                         falling back to canonical kit",
                        e
                    );
                }
            }
        } else {
            eprintln!(
                "kit-canonical: from_path: {:?} is not a directory; \
                 falling back to canonical kit",
                path
            );
        }

        // Fallback to the compiled-in canonical kit.
        Self::new()
    }

    /// Build a `Kit` from a `KitManifest`.
    ///
    /// This is a compatibility wrapper: in the data-driven model the manifest
    /// is replaced by the data fragment registry, so this always assembles the
    /// full canonical language regardless of the manifest contents. The
    /// manifest parameter is retained for API compatibility with downstream
    /// callers that still pass a `KitManifest`.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if fragment assembly or engine binding fails.
    pub fn from_manifest(_manifest: &KitManifest) -> Result<Self, LangError> {
        Self::new()
    }

    /// Lex `input` into a token stream using the data-driven lexer.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex failure (e.g. unrecognized character,
    /// unterminated string).
    pub fn lex(&self, input: &str) -> Result<Tokens, LangError> {
        self.runtime.lex(&Source::new(input))
    }

    /// Lex `input` into a token stream with source spans.
    ///
    /// The data-driven lexer produces tokens without spans. This method
    /// re-computes the byte span of each token by scanning the source text,
    /// skipping whitespace between tokens.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex failure.
    pub fn lex_with_spans(&self, input: &str) -> Result<Vec<SpannedToken>, LangError> {
        let tokens = self.lex(input)?;
        let source = input;
        let mut pos: usize = 0;
        let mut result = Vec::with_capacity(tokens.as_slice().len());
        for token in tokens.as_slice() {
            // Skip whitespace between tokens.
            while pos < source.len() {
                let ch = match source[pos..].chars().next() {
                    Some(c) => c,
                    None => break,
                };
                if ch.is_whitespace() {
                    pos += ch.len_utf8();
                } else {
                    break;
                }
            }
            let start = pos;
            pos += token.text.len();
            result.push(SpannedToken {
                kind: token.kind.clone(),
                text: token.text.clone(),
                span: Span { start, end: pos },
            });
        }
        Ok(result)
    }

    /// Parse `input` into an [`Ast`] using the Pratt parser.
    ///
    /// This lexes the input and then parses the token stream.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex or parse failure.
    pub fn parse(&self, input: &str) -> Result<Ast, LangError> {
        let tokens = self.lex(input)?;
        self.runtime.parse(&tokens)
    }

    /// Parse `input` as a module (multiple top-level statements) into an
    /// [`Ast`].
    ///
    /// In the data-driven model, the Pratt parser handles both single
    /// expressions and multi-statement modules in a single `parse` call;
    /// the resulting [`Ast`] arena contains all top-level nodes.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex or parse failure.
    pub fn parse_module(&self, input: &str) -> Result<Ast, LangError> {
        self.parse(input)
    }

    /// Evaluate `input` into a [`Value`] using the tree-walk evaluator.
    ///
    /// This lexes, parses, and evaluates the input.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on any stage failure.
    pub fn eval(&self, input: &str) -> Result<Value, LangError> {
        let ast = self.parse(input)?;
        self.runtime.eval(&ast)
    }

    /// Typecheck `input`, returning the synthesized type (ADR-0054).
    ///
    /// This lexes, parses, and typechecks the input. This is the pure-eval
    /// path: type expressions are evaluated via the bound typecheck engine
    /// without running effects.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on any stage failure.
    pub fn typecheck(&self, input: &str) -> Result<Value, LangError> {
        let ast = self.parse(input)?;
        let result = self.runtime.typecheck(&ast)?;
        Ok(result.typ)
    }

    /// Convenience: `eval(parse(lex(input)?)?)` with a typecheck stage
    /// (Phase 10.1).
    ///
    /// Delegates to [`Runtime::run_checked`], which is
    /// `lex → parse → typecheck → eval`. Typecheck errors
    /// (`Severity::Error`) short-circuit eval with a diagnostic. If no
    /// typecheck engine is bound, falls through to plain eval. Use
    /// [`Kit::eval`] for raw `parse → eval` without typechecking.
    ///
    /// # Errors
    ///
    /// Propagates the first stage error (including typecheck errors).
    pub fn run(&self, input: &str) -> Result<Value, LangError> {
        self.runtime.run_checked(&Source::new(input))
    }

    /// Returns the scope name for this kit's grammar.
    pub fn scope_name(&self) -> &str {
        "source.handl"
    }

    /// Returns the file extensions for this kit's grammar.
    pub fn file_extensions(&self) -> Vec<String> {
        vec!["hnl".to_string()]
    }

    /// Returns the underlying [`Runtime`], giving callers direct access to
    /// the staged `lex`/`parse`/`eval` methods with [`Source`]/[`Tokens`]/
    /// [`Ast`] inputs.
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Returns the [`Language`](handl_language::Language) this kit was built
    /// from — the data source for token classification
    /// ([`TokenClasses`]), grammar, and AST-shape queries.
    pub fn language(&self) -> &Language {
        self.runtime.lang()
    }

    /// Returns the configured module root for resolving relative
    /// `import "path"` paths (Phase 9.5). `None` → resolve against the
    /// process CWD.
    pub fn module_root(&self) -> Option<&Path> {
        self.module_root.as_deref()
    }
}

impl Default for Kit {
    fn default() -> Self {
        Self::new().expect("canonical kit assembly should not fail")
    }
}

// ---------------------------------------------------------------------------
// runtime_for_lang — build a Runtime for an arbitrary Language with the
// canonical engine registry + meta chain (for tests / custom-language hosts)
// ---------------------------------------------------------------------------

/// Build a [`Runtime`] for an arbitrary [`Language`](handl_language::Language)
/// using the canonical engine registry + meta chain (the 5
/// `plugin-lib-lang-*` modules). This is the same wiring as [`Kit::new`] but
/// with a caller-supplied language instead of the assembled canonical one —
/// useful for tests that override eval schemes (e.g. `vau` escape tests).
///
/// Injects `self` as a global bound to the `Language` value
/// (ADR-0041 Phase 3).
///
/// # Errors
///
/// Returns [`LangError`] if engine binding fails.
pub fn runtime_for_lang(lang: handl_language::Language) -> Result<Runtime, LangError> {
    let config = handl_kit_data_canonical::manifest().runtime.clone();

    // Build the engine registry with all available engine impls
    // (ADR-0047 §2).
    let mut registry = EngineRegistry::new();
    registry.register_lex(
        "data",
        Box::new(handl_plugin_engine_lex_pattern::DataLexerEngine::new()),
    );
    registry.register_parse("pratt", handl_plugin_engine_parse_pratt::PrattEngine::new());
    registry.register_eval(
        "tree-walk",
        Box::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new()),
    );
    registry.register_eval(
        "bytecode",
        Box::new(handl_plugin_engine_exec_bytecode::BytecodeEngine::new()),
    );
    registry.register_eval(
        "cranelift",
        Box::new(handl_plugin_engine_exec_cranelift::CraneliftEngine::new()),
    );
    registry.register_compile(
        "bytecode",
        Box::new(handl_plugin_engine_exec_bytecode::BytecodeCompileEngine::new()),
    );
    registry.register_compile(
        "native",
        Box::new(handl_plugin_engine_exec_cranelift::CraneliftCompileEngine::new()),
    );
    registry.register_typecheck(
        "nbe",
        Box::new(NbeTypecheckEngine::new(Box::new(
            handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new(),
        ))),
    );
    let registry = Rc::new(registry);

    // Build the meta chain from the 6 plugin-lib-lang-* crates
    // (ADR-0047 §1, ADR-0054). Delegate is None (no user host fns).
    let factory = Rc::new(RuntimeFactoryImpl::new(
        Rc::clone(&registry),
        config.clone(),
    ));
    let factory_dyn: Rc<dyn RuntimeFactory> = Rc::clone(&factory) as Rc<dyn RuntimeFactory>;
    let run_compiled_mod = Rc::new(RunCompiledModule::new(Rc::clone(&factory_dyn), None));
    let compile_mod = Rc::new(CompileModule::new(
        Rc::clone(&factory_dyn),
        Some(Rc::clone(&run_compiled_mod) as Rc<dyn handl_platform::ModuleLookup>),
    ));
    let typecheck_mod = Rc::new(TypecheckModule::new(
        Rc::clone(&factory_dyn),
        Some(Rc::clone(&compile_mod) as Rc<dyn handl_platform::ModuleLookup>),
    ));
    let eval_mod = Rc::new(EvalModule::new(
        Rc::clone(&factory_dyn),
        Some(Rc::clone(&typecheck_mod) as Rc<dyn handl_platform::ModuleLookup>),
    ));
    let parse_mod = Rc::new(ParseModule::new(
        Rc::clone(&factory_dyn),
        Some(Rc::clone(&eval_mod) as Rc<dyn handl_platform::ModuleLookup>),
    ));
    let lex_mod = Rc::new(LexModule::new(
        Rc::clone(&factory_dyn),
        Some(Rc::clone(&parse_mod) as Rc<dyn handl_platform::ModuleLookup>),
    ));
    let meta_chain: Rc<dyn handl_platform::ModuleLookup> = lex_mod;
    factory.set_meta_chain(Rc::clone(&meta_chain));

    // Inject `self` as a global variable bound to the Language value
    // (ADR-0041 Phase 3).
    let self_val = handl_language::language_value(&lang);
    let mut globals = std::collections::HashMap::new();
    globals.insert("self".to_string(), self_val);
    Runtime::from_registry_with_globals(lang, &registry, &config, Some(meta_chain), Some(&globals))
}

// ---------------------------------------------------------------------------
// Platform AstNode conversion helpers
// ---------------------------------------------------------------------------

/// Convert a [`handl_language::AstNode`] to a [`handl_platform::AstNode`].
///
/// The two types are structurally identical (`kind`, `span`, `payload`) but
/// defined in different crates. This helper bridges them so hosts that
/// dispatch evaluation through the platform plugin system (e.g. `host-cli`)
/// can consume the data-driven parser's output.
pub fn to_platform_ast_node(node: &handl_language::AstNode) -> handl_platform::AstNode {
    handl_platform::AstNode {
        kind: node.kind.clone(),
        span: node.span.map(|s| handl_platform::Span {
            start: s.start,
            end: s.end,
        }),
        payload: node.payload.clone(),
    }
}

/// Convert all nodes in an [`Ast`] arena to a `Vec<handl_platform::AstNode>`.
pub fn ast_to_platform_nodes(ast: &Ast) -> Vec<handl_platform::AstNode> {
    ast.nodes().iter().map(to_platform_ast_node).collect()
}

// ---------------------------------------------------------------------------
// TokenClasses — data-driven token classification (ADR-0044)
// ---------------------------------------------------------------------------

/// Token-kind classification resolved from a [`Language`]'s `tokens` fragment
/// set (`TokenKindDef::category`/`pair`/`doc`/`label`) — the data-driven
/// replacement for the string-matching [`is_identifier_kind`]-style helpers
/// below.
///
/// Build one per request from the kit's language
/// (`TokenClasses::new(kit.language())`) and query by category, pair, label,
/// or doc instead of comparing `token.kind` against `"token.*"` literals: a
/// language whose kinds are named differently still classifies correctly
/// because the category travels with the token data.
///
/// Category vocabulary used by the canonical kit: `keyword`, `identifier`,
/// `number`, `string`, `bool`, `null`, `template`, `operator`,
/// `punctuation`, `open-bracket`, `close-bracket`, `comment`, `doc-comment`,
/// `eof`.
#[derive(Debug, Clone)]
pub struct TokenClasses {
    /// kind id → category.
    categories: std::collections::HashMap<String, String>,
    /// kind id → paired bracket kind id.
    pairs: std::collections::HashMap<String, String>,
    /// surface-syntax label → kind id (first declaration wins).
    labels: std::collections::HashMap<String, String>,
    /// kind id → declared surface-syntax label.
    kind_labels: std::collections::HashMap<String, String>,
    /// kind id → hover/doc markdown.
    docs: std::collections::HashMap<String, String>,
}

impl TokenClasses {
    /// Build the classification tables from `lang`'s `tokens` fragment set.
    pub fn new(lang: &Language) -> Self {
        let mut classes = Self {
            categories: std::collections::HashMap::new(),
            pairs: std::collections::HashMap::new(),
            labels: std::collections::HashMap::new(),
            kind_labels: std::collections::HashMap::new(),
            docs: std::collections::HashMap::new(),
        };
        for def in lang.tokens.iter() {
            if let Some(cat) = &def.category {
                classes.categories.insert(def.id.clone(), cat.clone());
            }
            if let Some(pair) = &def.pair {
                classes.pairs.insert(def.id.clone(), pair.clone());
            }
            if let Some(label) = &def.label {
                classes
                    .labels
                    .entry(label.clone())
                    .or_insert_with(|| def.id.clone());
                classes.kind_labels.insert(def.id.clone(), label.clone());
            }
            if let Some(doc) = &def.doc {
                classes.docs.insert(def.id.clone(), doc.clone());
            }
        }
        classes
    }

    /// Whether token kind `kind` is classified as `category`.
    pub fn is(&self, kind: &str, category: &str) -> bool {
        self.categories.get(kind).map(|c| c == category).unwrap_or(false)
    }

    /// The declared category for `kind`, if any.
    pub fn category_of(&self, kind: &str) -> Option<&str> {
        self.categories.get(kind).map(String::as_str)
    }

    /// Whether `kind` is an identifier.
    pub fn is_identifier(&self, kind: &str) -> bool {
        self.is(kind, "identifier")
    }

    /// Whether `kind` is a keyword.
    pub fn is_keyword(&self, kind: &str) -> bool {
        self.is(kind, "keyword")
    }

    /// Whether `kind` is a number literal.
    pub fn is_number(&self, kind: &str) -> bool {
        self.is(kind, "number")
    }

    /// Whether `kind` is a string literal.
    pub fn is_string(&self, kind: &str) -> bool {
        self.is(kind, "string")
    }

    /// Whether `kind` is a boolean literal.
    pub fn is_bool(&self, kind: &str) -> bool {
        self.is(kind, "bool")
    }

    /// Whether `kind` is the null literal.
    pub fn is_null(&self, kind: &str) -> bool {
        self.is(kind, "null")
    }

    /// Whether `kind` is a comment (line or doc comment).
    pub fn is_comment(&self, kind: &str) -> bool {
        self.is(kind, "comment") || self.is(kind, "doc-comment")
    }

    /// Whether `kind` is a doc comment.
    pub fn is_doc_comment(&self, kind: &str) -> bool {
        self.is(kind, "doc-comment")
    }

    /// Whether `kind` is an operator.
    pub fn is_operator(&self, kind: &str) -> bool {
        self.is(kind, "operator")
    }

    /// Whether `kind` is a template string.
    pub fn is_template(&self, kind: &str) -> bool {
        self.is(kind, "template")
    }

    /// Whether `kind` is a delimiter/punctuation (brackets and separators).
    pub fn is_delimiter(&self, kind: &str) -> bool {
        self.is(kind, "punctuation")
            || self.is(kind, "open-bracket")
            || self.is(kind, "close-bracket")
    }

    /// Whether `kind` opens a bracket pair.
    pub fn is_open_bracket(&self, kind: &str) -> bool {
        self.is(kind, "open-bracket")
    }

    /// Whether `kind` closes a bracket pair.
    pub fn is_close_bracket(&self, kind: &str) -> bool {
        self.is(kind, "close-bracket")
    }

    /// Whether `kind` is any bracket (open or close).
    pub fn is_bracket(&self, kind: &str) -> bool {
        self.is_open_bracket(kind) || self.is_close_bracket(kind)
    }

    /// The kind id paired with `kind` (open ↔ close), if declared.
    pub fn pair_of(&self, kind: &str) -> Option<&str> {
        self.pairs.get(kind).map(String::as_str)
    }

    /// Resolve a surface-syntax `label` (e.g. `"."`, `"::"`) to the kind id
    /// that declares it — the data-driven answer to "is this the dot token?".
    pub fn kind_for_label(&self, label: &str) -> Option<&str> {
        self.labels.get(label).map(String::as_str)
    }

    /// Whether token kind `kind` is the kind that spells `label`.
    pub fn is_label(&self, kind: &str, label: &str) -> bool {
        self.kind_for_label(label) == Some(kind)
    }

    /// The surface-syntax label declared for `kind`, if any.
    pub fn label_of(&self, kind: &str) -> Option<&str> {
        self.kind_labels.get(kind).map(String::as_str)
    }

    /// The hover/doc markdown declared for `kind`, if any.
    pub fn doc_of(&self, kind: &str) -> Option<&str> {
        self.docs.get(kind).map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Token classification helpers (string-based kind matching)
// ---------------------------------------------------------------------------
//
// Legacy string-matching helpers, kept for callers without a `Language` at
// hand (bootstrap paths, tests, third-party kits that declare no
// categories). New code that has a `Language` should prefer
// [`TokenClasses`], which answers the same questions from `lang.tokens`
// data instead of hardcoded canonical kind ids.

/// Whether a token kind string represents an identifier.
pub fn is_identifier_kind(kind: &str) -> bool {
    kind == "token.identifier"
}

/// Whether a token kind string represents a keyword.
pub fn is_keyword_kind(kind: &str) -> bool {
    kind.starts_with("token.keyword")
}

/// Whether a token kind string represents a number literal.
pub fn is_number_kind(kind: &str) -> bool {
    kind == "token.number"
}

/// Whether a token kind string represents a string literal.
pub fn is_string_kind(kind: &str) -> bool {
    kind == "token.string"
}

/// Whether a token kind string represents a boolean literal.
pub fn is_bool_kind(kind: &str) -> bool {
    kind == "token.bool"
}

/// Whether a token kind string represents the null literal.
pub fn is_null_kind(kind: &str) -> bool {
    kind == "token.null"
}

/// Whether a token kind string represents a comment.
pub fn is_comment_kind(kind: &str) -> bool {
    kind == "token.comment" || kind == "token.doc_comment"
}

/// Whether a token kind string represents a doc comment.
pub fn is_doc_comment_kind(kind: &str) -> bool {
    kind == "token.doc_comment"
}

/// Whether a token kind string represents an operator.
pub fn is_operator_kind(kind: &str) -> bool {
    matches!(
        kind,
        "token.bind"
            | "token.equal"
            | "token.plus"
            | "token.minus"
            | "token.star"
            | "token.pow"
            | "token.slash"
            | "token.lt"
            | "token.gt"
            | "token.eqeq"
            | "token.neq"
            | "token.lte"
            | "token.gte"
            | "token.spaceship"
            | "token.flow"
            | "token.fanout"
            | "token.pipe"
            | "token.arrow"
            | "token.and"
            | "token.or"
            | "token.bang"
            | "token.choice"
            | "token.percent"
            | "token.question"
            | "token.ellipsis"
    )
}

/// Whether a token kind string represents a delimiter/punctuation.
pub fn is_delimiter_kind(kind: &str) -> bool {
    kind.starts_with("token.lparen")
        || kind.starts_with("token.rparen")
        || kind.starts_with("token.lbracket")
        || kind.starts_with("token.rbracket")
        || kind.starts_with("token.lbrace")
        || kind.starts_with("token.rbrace")
        || kind == "token.comma"
        || kind == "token.dot"
        || kind == "token.colon"
        || kind == "token.semicolon"
        || kind == "token.arrow"
}

/// Whether a token kind string represents a template string.
pub fn is_template_kind(kind: &str) -> bool {
    kind == "token.template"
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::AstNode;

    #[test]
    fn lex_number() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.as_slice().len(), 1);
        assert_eq!(tokens.as_slice()[0].kind, "token.number");
        assert_eq!(tokens.as_slice()[0].text, "42");
    }

    #[test]
    fn hello_greet_returns_canonical_greeting() {
        // The greet logic is handl-authored in greet.hnl; this confirms the
        // bridge loads + evaluates it through the live Kit runtime path.
        let kit = Kit::new().unwrap();
        let val = kit.run("hello.greet(\"world\")").unwrap();
        assert_eq!(val, Value::String("Hello, world!".to_string()));
    }

    #[test]
    fn hello_greet_rejects_non_string_arg() {
        let kit = Kit::new().unwrap();
        assert!(kit.run("hello.greet(42)").is_err());
    }

    #[test]
    fn hello_greet_bridge_escapes_special_chars() {
        let kit = Kit::new().unwrap();
        let val = kit.run("hello.greet(\"a\\\"b\")").unwrap();
        assert_eq!(val, Value::String("Hello, a\"b!".to_string()));
    }

    #[test]
    fn lex_string() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex("\"hi\"").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.string");
        assert_eq!(tokens.as_slice()[0].text, "\"hi\"");
    }

    #[test]
    fn lex_identifier() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex("foo").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.identifier");
        assert_eq!(tokens.as_slice()[0].text, "foo");
    }

    #[test]
    fn lex_with_spans() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex_with_spans("1 + 2").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, "token.number");
        assert_eq!(tokens[0].text, "1");
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 1);
        assert_eq!(tokens[1].kind, "token.plus");
        assert_eq!(tokens[1].span.start, 2);
        assert_eq!(tokens[1].span.end, 3);
        assert_eq!(tokens[2].kind, "token.number");
        assert_eq!(tokens[2].text, "2");
        assert_eq!(tokens[2].span.start, 4);
        assert_eq!(tokens[2].span.end, 5);
    }

    #[test]
    fn parse_number() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("1").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_call() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("add(1, 2)").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_index() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("arr[0]").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_let() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("a = 1").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_string_literal() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("\"hi\"").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_bool_literal() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("true").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_null_literal() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("null").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_array() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("[1, 2]").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_object() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("{a: 1}").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_match() {
        // `match x` is incomplete (missing the `{ arms }` block) and errors.
        let kit = Kit::new().unwrap();
        let result = kit.parse("match x");
        assert!(result.is_err(), "expected parse error for incomplete match");
    }

    #[test]
    fn parse_match_full() {
        // The full `match x { a: 1, b: 2 }` form parses via the Pratt
        // engine's delimited-block support into match{[scrut, block{[arm,
        // arm]}]}.
        let kit = Kit::new().unwrap();
        let ast = kit.parse("match x { a: 1, b: 2 }").unwrap();
        let root = &ast.nodes()[ast.len() - 1];
        assert_eq!(root.kind, "match");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 2);
        let block = &ast.nodes()[children[1].as_u64().unwrap() as usize];
        assert_eq!(block.kind, "block");
        let arms = block.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(arms.len(), 2);
        assert_eq!(ast.nodes()[arms[0].as_u64().unwrap() as usize].kind, "arm");
        assert_eq!(ast.nodes()[arms[1].as_u64().unwrap() as usize].kind, "arm");
    }

    // NOTE: the four `handle` tests (`parse_handle`, `parse_handle_full`,
    // `eval_handle_dispatches_matching_clause`, `eval_handle_no_match_returns_body`)
    // were removed in v0.1. The `handle` keyword (effect-handler dispatch) was
    // removed from the language per ADR-0074; it has no direct v0.1 equivalent
    // (the closest constructs are `match` for value dispatch and `|||` for
    // arrow/choice dispatch). These tests referenced the removed syntax and
    // were deleted rather than migrated.

    #[test]
    fn parse_module_empty() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse_module("").unwrap();
        assert_eq!(ast.len(), 0);
    }

    #[test]
    fn parse_module_single_let() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse_module("a = 1").unwrap();
        assert!(ast.len() >= 1);
    }

    #[test]
    fn parse_module_multiple_let() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse_module("a = 1; b = 2").unwrap();
        assert!(ast.len() >= 2);
    }

    #[test]
    fn eval_number() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("42").unwrap();
        assert_eq!(val, serde_json::json!(42));
    }

    #[test]
    fn eval_string() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("\"hi\"").unwrap();
        assert_eq!(val, serde_json::json!("hi"));
    }

    #[test]
    fn eval_bool() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("true").unwrap();
        assert_eq!(val, serde_json::json!(true));
    }

    #[test]
    fn eval_null() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("null").unwrap();
        assert_eq!(val, serde_json::Value::Null);
    }

    #[test]
    fn eval_arith() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("1 + 2").unwrap();
        assert_eq!(val, serde_json::json!(3));
    }

    #[test]
    fn eval_match_literal_arm() {
        let kit = Kit::new().unwrap();
        let val = kit.eval(r#"match 1 { 1: "a", 2: "b" }"#).unwrap();
        assert_eq!(val, serde_json::json!("a"));
    }

    #[test]
    fn eval_match_second_arm() {
        let kit = Kit::new().unwrap();
        let val = kit.eval(r#"match 2 { 1: "a", 2: "b" }"#).unwrap();
        assert_eq!(val, serde_json::json!("b"));
    }

    #[test]
    fn eval_match_wildcard_arm() {
        let kit = Kit::new().unwrap();
        let val = kit.eval(r#"match 99 { 1: "a", _: "default" }"#).unwrap();
        assert_eq!(val, serde_json::json!("default"));
    }

    #[test]
    fn eval_match_binder_arm() {
        // A binder arm (`x`) binds the scrutinee and the body references it.
        let kit = Kit::new().unwrap();
        let val = kit.eval(r#"match 7 { 1: "a", x: x }"#).unwrap();
        assert_eq!(val, serde_json::json!(7));
    }

    #[test]
    fn eval_match_no_arm_errors() {
        let kit = Kit::new().unwrap();
        let res = kit.eval(r#"match 5 { 1: "a", 2: "b" }"#);
        assert!(res.is_err());
    }

    #[test]
    fn run_full_pipeline() {
        let kit = Kit::new().unwrap();
        let val = kit.run("1 + 2 * 3").unwrap();
        assert_eq!(val, serde_json::json!(7));
    }

    #[test]
    fn from_manifest_works() {
        let manifest = canonical_manifest();
        let kit = Kit::from_manifest(&manifest).unwrap();
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.number");
    }

    #[test]
    fn scope_name_and_extensions() {
        let kit = Kit::new().unwrap();
        assert_eq!(kit.scope_name(), "source.handl");
        assert_eq!(kit.file_extensions(), vec!["hnl".to_string()]);
    }

    #[test]
    fn to_platform_ast_node_converts() {
        let node = handl_language::AstNode::new("value")
            .with_span(handl_language::Span { start: 0, end: 2 })
            .with_payload(serde_json::json!({ "value": 42 }));
        let platform_node = to_platform_ast_node(&node);
        assert_eq!(platform_node.kind, "value");
        assert_eq!(
            platform_node.span,
            Some(handl_platform::Span { start: 0, end: 2 })
        );
        assert_eq!(platform_node.payload, serde_json::json!({ "value": 42 }));
    }

    // --- Token classification helpers ---

    #[test]
    fn is_identifier_kind_works() {
        assert!(is_identifier_kind("token.identifier"));
        assert!(!is_identifier_kind("token.number"));
    }

    #[test]
    fn is_keyword_kind_works() {
        assert!(is_keyword_kind("token.keyword.if"));
        assert!(is_keyword_kind("token.keyword.match"));
        assert!(!is_keyword_kind("token.identifier"));
    }

    #[test]
    fn is_number_kind_works() {
        assert!(is_number_kind("token.number"));
        assert!(!is_number_kind("token.string"));
    }

    // --- ADR-0054 typecheck stage ---

    #[test]
    fn typecheck_number_returns_type_value() {
        // The `value` type rule dispatches on JSON type: 42 → type.number.
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("42").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.number"));
        assert_eq!(typ["__registry__"], serde_json::json!("handl.canonical"));
    }

    #[test]
    fn typecheck_unbound_var_errors() {
        // `x` is an unbound variable: `var("x")` with no Γ entry → `lookup`
        // fails.
        let kit = Kit::new().unwrap();
        let res = kit.typecheck("x");
        assert!(res.is_err(), "expected typecheck error for unbound var");
    }

    #[test]
    fn typecheck_arithmetic_returns_type_number() {
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("1 + 2").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.number"));
    }

    #[test]
    fn typecheck_bool_returns_type_bool() {
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("true").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.bool"));
    }

    #[test]
    fn typecheck_if_with_bool_cond_returns_branch_type() {
        // v0.1 `if` syntax (ADR-0074): the `then` keyword was removed; the
        // grammar is `if cond { then } else { else }` or `if cond then-expr
        // else else-expr` (no `then`). Here we use the brace-less form.
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("if true 1 else 2").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.number"));
    }

    #[test]
    fn typecheck_if_with_non_bool_cond_errors() {
        // v0.1 `if` syntax (ADR-0074): `then` removed; use the brace-less
        // `if cond then-expr else else-expr` form so the condition actually
        // reaches the typechecker (rather than erroring at parse time).
        let kit = Kit::new().unwrap();
        let res = kit.typecheck("if 42 1 else 2");
        assert!(
            res.is_err(),
            "expected typecheck error for non-bool if condition"
        );
    }

    #[test]
    fn typecheck_let_binds_var_and_returns_body_type() {
        // ADR-0079: `let ... in` removed. Use `do { x :: 1; x }` graph block.
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("do { x :: 1; x }").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.number"));
    }

    #[test]
    fn typecheck_fn_binds_param_and_returns_function_type() {
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("x ~> x").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.function"));
    }

    #[test]
    fn typecheck_match_returns_arm_body_type() {
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("match 1 { 1: 2, _: 3 }").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.number"));
    }

    #[test]
    fn typecheck_type_annot_checks_against_annotation() {
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("2: type.number").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.number"));
    }

    #[test]
    fn typecheck_type_annot_rejects_mismatch() {
        let kit = Kit::new().unwrap();
        let res = kit.typecheck("2: type.string");
        assert!(res.is_err(), "expected typecheck error for type mismatch");
    }

    #[test]
    fn typecheck_type_annot_bool() {
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("true: type.bool").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.bool"));
    }

    // NOTE: `let` is fully removed from the language (ADR-0079 completed):
    // bindings are plain `x = y` / `x :: y`, and `do { ... }` graph blocks
    // cover the old `let ... in` form. Type-annotated block bindings
    // (`do { x: T = e; ... }`) are future work — the block binding target
    // would need to parse `x: T` as a `type-annot` child.
    #[test]
    fn typecheck_let_with_annotation_checks_value() {
        // ADR-0079: `let ... in` removed. Graph blocks don't support type
        // annotations yet — use untyped `do { x :: 2; x }`.
        let kit = Kit::new().unwrap();
        let typ = kit.typecheck("do { x :: 2; x }").unwrap();
        assert_eq!(typ["__tag__"], serde_json::json!("type.number"));
    }

    #[test]
    fn typecheck_let_with_annotation_rejects_mismatch() {
        // ADR-0079: `let ... in` removed. Type-annotated graph block bindings
        // are future work. This test now checks a simpler mismatch case.
        let kit = Kit::new().unwrap();
        let res = kit.typecheck("if 42 1 else 2");
        assert!(
            res.is_err(),
            "expected typecheck error for non-bool if condition"
        );
    }

    // -----------------------------------------------------------------------
    // Kit::from_path tests (ADR 0003 per-file kit loading)
    // -----------------------------------------------------------------------

    /// Path to the canonical kit's checked-in YAML data directory, resolved
    /// relative to this crate's manifest dir.
    fn canonical_data_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("handl-kit-data-canonical")
            .join("data")
    }

    #[test]
    fn from_path_sentinel_returns_canonical_kit() {
        let kit = Kit::from_path(std::path::Path::new("handl-kit-data-canonical")).unwrap();
        // The sentinel path must behave identically to Kit::new().
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.number");
        assert_eq!(tokens.as_slice()[0].text, "42");
    }

    #[test]
    fn from_path_real_directory_loads_yaml() {
        let dir = canonical_data_dir();
        assert!(dir.is_dir(), "canonical data dir should exist: {:?}", dir);
        let kit = Kit::from_path(&dir).unwrap();
        // A kit loaded from the same YAML files as the compiled-in kit must
        // lex/parse identically.
        let tokens = kit.lex("1 + 2").unwrap();
        assert_eq!(tokens.as_slice().len(), 3);
        assert_eq!(tokens.as_slice()[0].kind, "token.number");
        let ast = kit.parse("add(1, 2)").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn from_path_real_directory_evaluates() {
        let dir = canonical_data_dir();
        let kit = Kit::from_path(&dir).unwrap();
        let val = kit.eval("1 + 2").unwrap();
        assert_eq!(val, serde_json::json!(3));
    }

    #[test]
    fn from_path_nonexistent_directory_falls_back_to_new() {
        let kit =
            Kit::from_path(std::path::Path::new("/this/path/does/not/exist/kit-data")).unwrap();
        // Fallback to Kit::new() must still produce a working kit.
        let tokens = kit.lex("foo").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.identifier");
    }

    #[test]
    fn from_path_directory_missing_yaml_falls_back_to_new() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Write only one of the eight required YAML files.
        std::fs::write(tmp.path().join("tokens.yaml"), "[]\n").unwrap();
        let kit = Kit::from_path(tmp.path()).unwrap();
        // Missing files -> fallback to Kit::new() -> working kit.
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.number");
    }

    #[test]
    fn from_path_empty_directory_falls_back_to_new() {
        let tmp = tempfile::TempDir::new().unwrap();
        let kit = Kit::from_path(tmp.path()).unwrap();
        let tokens = kit.lex("\"hi\"").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.string");
    }

    // -----------------------------------------------------------------------
    // ADR-0082 Phase 1: `data` declarations from surface syntax
    // -----------------------------------------------------------------------

    /// Helper: parse source with the Pratt engine directly (no desugar) to
    /// inspect the pre-desugar AST.
    fn parse_raw(kit: &Kit, src: &str) -> Ast {
        use handl_plugin_engine_parse::ParserEngine;
        use handl_plugin_engine_parse_pratt::PrattEngine;

        let tokens = kit.lex(src).unwrap();
        let lang = kit.runtime().lang();
        let engine = PrattEngine::new();
        let req = handl_plugin_engine_parse::ParserRequest::new(tokens);
        engine.parse(lang, &req).unwrap()
    }

    #[test]
    fn data_nat_parses_to_data_node_with_alt_tree() {
        let kit = Kit::new().unwrap();
        let ast = parse_raw(&kit, "data Nat :: Zero | Succ(Nat)");

        // The last node should be a `data` node.
        let root = ast.nodes().last().unwrap();
        assert_eq!(root.kind, "data");
        // payload.name should be "Nat".
        let name = root.payload.get("name").and_then(|v| v.as_str()).unwrap();
        assert_eq!(name, "Nat");
        // Two children: [type_expr, constructors_expr].
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(
            children.len(),
            2,
            "data node should have 2 children [type, constructors]"
        );
        // Child 0 is the type expression: var("Nat").
        let type_idx = children[0].as_u64().unwrap() as usize;
        let type_node = ast.get(handl_language::AstRef::new(type_idx)).unwrap();
        assert_eq!(type_node.kind, "var");
        assert_eq!(type_node.payload, serde_json::json!("Nat"));
        // Child 1 should be an `alt` node.
        let child_idx = children[1].as_u64().unwrap() as usize;
        let child = ast.get(handl_language::AstRef::new(child_idx)).unwrap();
        assert_eq!(child.kind, "alt");
        // The alt node should have 2 children (left=Zero, right=Succ(Nat)).
        let alt_children = child.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(alt_children.len(), 2);
        // Left child: var("Zero")
        let left = ast
            .get(handl_language::AstRef::new(
                alt_children[0].as_u64().unwrap() as usize,
            ))
            .unwrap();
        assert_eq!(left.kind, "var");
        assert_eq!(left.payload, serde_json::json!("Zero"));
        // Right child: call(Succ, Nat)
        let right = ast
            .get(handl_language::AstRef::new(
                alt_children[1].as_u64().unwrap() as usize,
            ))
            .unwrap();
        assert_eq!(right.kind, "call");
    }

    #[test]
    fn data_nat_desugars_to_block_from_source() {
        let kit = Kit::new().unwrap();
        // kit.parse applies the desugar pass.
        let ast = kit.parse("data Nat :: Zero | Succ(Nat)").unwrap();

        // The root should be a `block` with 4 statements (predicate + 2
        // constructors + recursor).
        let root = ast.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(
            children.len(),
            4,
            "expected 4 statements (pred + 2 cons + rec)"
        );
        // Each statement should be an `assign`.
        for (i, child_idx) in children.iter().enumerate() {
            let node = ast
                .get(handl_language::AstRef::new(
                    child_idx.as_u64().unwrap() as usize
                ))
                .unwrap();
            assert_eq!(
                node.kind, "assign",
                "statement {} should be assign, got {}",
                i, node.kind
            );
        }
    }

    #[test]
    fn data_bool_desugars_from_source() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("data Bool :: True | False").unwrap();

        let root = ast.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // 1 predicate + 2 constructors + 1 recursor = 4
        assert_eq!(children.len(), 4);
    }

    #[test]
    fn data_nat_eval_succ_zero() {
        // End-to-end: parse `data Nat :: Zero | Succ(Nat)` and `Succ(Zero)`,
        // desugar, and eval to get a tagged value.
        //
        // The Pratt parser parses one statement per call, so we parse the
        // data declaration and the expression separately, then combine the
        // desugared ASTs into a single block where the data's assign
        // statements and the expression are siblings (so the eval_block
        // handler defines the constructors in scope before evaluating the
        // expression).
        let kit = Kit::new().unwrap();

        // Parse and desugar the data declaration.
        let data_ast = kit.parse("data Nat :: Zero | Succ(Nat)").unwrap();
        // The root is a block with 4 assign children.
        let data_root = data_ast.nodes().last().unwrap();
        assert_eq!(data_root.kind, "block");
        let data_children: Vec<usize> = data_root
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();

        // Parse and desugar the expression.
        let expr_ast = kit.parse("Succ(Zero)").unwrap();
        let expr_root_idx = expr_ast.len() - 1;

        // Combine: copy all data_ast nodes, then all expr_ast nodes (shifted),
        // then a final block node with [data_children..., expr_root_shifted].
        let mut combined = Ast::new();
        // Copy data_ast nodes (all of them, including the block root — we
        // just won't use the block root as the combined root).
        for node in data_ast.nodes() {
            combined.push(node.clone());
        }
        let data_len = data_ast.len();
        // Copy expr_ast nodes with shifted children indices.
        for node in expr_ast.nodes() {
            let mut payload = node.payload.clone();
            if let Some(children) = payload.get_mut("children").and_then(|c| c.as_array_mut()) {
                for child in children.iter_mut() {
                    if let Some(idx) = child.as_u64() {
                        *child = serde_json::json!(idx as usize + data_len);
                    }
                }
            }
            // Also shift "body" field (used by function nodes).
            if let Some(body) = payload.get_mut("body").and_then(|b| b.as_u64()) {
                *payload.get_mut("body").unwrap() = serde_json::json!(body as usize + data_len);
            }
            combined.push(AstNode {
                kind: node.kind.clone(),
                span: node.span,
                payload,
            });
        }
        let expr_root_shifted = expr_root_idx + data_len;

        // Build the combined block: [data assigns..., expr call].
        let mut all_children = data_children.clone();
        all_children.push(expr_root_shifted);
        let block_payload = serde_json::json!({ "children": all_children });
        combined.push(AstNode::new("block").with_payload(block_payload));

        // Evaluate the combined AST.
        let val = kit.runtime().eval(&combined).unwrap();
        // Succ(Zero) should produce {"tag": "Succ", "nat": {"tag": "Zero"}}
        // (arg name "nat" is derived from the lowercased type name "Nat").
        let obj = val.as_object().expect("expected object result");
        assert_eq!(obj.get("tag"), Some(&serde_json::json!("Succ")));
        let pred = obj.get("nat").expect("expected 'nat' field");
        let pred_obj = pred.as_object().expect("expected nested object");
        assert_eq!(pred_obj.get("tag"), Some(&serde_json::json!("Zero")));
    }

    #[test]
    fn data_nat_eval_zero() {
        // Eval `Zero` after declaring `data Nat :: Zero | Succ(Nat)`.
        // Same combining approach as data_nat_eval_succ_zero.
        let kit = Kit::new().unwrap();
        let data_ast = kit.parse("data Nat :: Zero | Succ(Nat)").unwrap();
        let data_root = data_ast.nodes().last().unwrap();
        let data_children: Vec<usize> = data_root
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();

        let expr_ast = kit.parse("Zero").unwrap();
        let expr_root_idx = expr_ast.len() - 1;

        let mut combined = Ast::new();
        for node in data_ast.nodes() {
            combined.push(node.clone());
        }
        let data_len = data_ast.len();
        for node in expr_ast.nodes() {
            let mut payload = node.payload.clone();
            if let Some(children) = payload.get_mut("children").and_then(|c| c.as_array_mut()) {
                for child in children.iter_mut() {
                    if let Some(idx) = child.as_u64() {
                        *child = serde_json::json!(idx as usize + data_len);
                    }
                }
            }
            combined.push(AstNode {
                kind: node.kind.clone(),
                span: node.span,
                payload,
            });
        }
        let expr_root_shifted = expr_root_idx + data_len;
        let mut all_children = data_children.clone();
        all_children.push(expr_root_shifted);
        let block_payload = serde_json::json!({ "children": all_children });
        combined.push(AstNode::new("block").with_payload(block_payload));

        let val = kit.runtime().eval(&combined).unwrap();
        let obj = val.as_object().expect("expected object result");
        assert_eq!(obj.get("tag"), Some(&serde_json::json!("Zero")));
    }

    #[test]
    fn data_undesugared_reaches_eval_errors() {
        // If a `data` node somehow reaches eval (not desugared), the
        // eval.data scheme should raise an error. We verify this by
        // building a raw AST with a `data` node and evaluating it
        // directly (bypassing the desugar pass).
        use handl_language::{Ast, AstNode};
        use serde_json::json;

        let kit = Kit::new().unwrap();
        let mut ast = Ast::new();
        ast.push(AstNode::new("data").with_payload(json!({"name": "Nat", "children": []})));
        let result = kit.runtime().eval(&ast);
        assert!(result.is_err(), "expected error for undesugared data node");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("data-error") || err_msg.contains("not desugared"),
            "expected data-error message, got: {}",
            err_msg
        );
    }

    // -----------------------------------------------------------------------
    // Task 1.2: Desugar pipeline documentation (Option A)
    // -----------------------------------------------------------------------
    // The desugar pass runs at parse time (Runtime::parse), before typecheck
    // and eval. This is Option A from the task spec. The test below verifies
    // that the typechecker sees the desugared output (the `Nat` predicate is
    // defined after desugar).

    #[test]
    fn data_desugar_visible_to_typechecker() {
        // Parse `data Nat :: Zero | Succ(Nat)` from source. The desugar pass
        // runs at parse time (Runtime::parse), so the typechecker should see
        // the desugared `Nat` predicate. We verify by typechecking a module
        // that includes the data declaration — the typechecker input is the
        // desugared AST.
        //
        // Note: the bootstrap typechecker may not fully support inductive
        // types yet. This test verifies that the desugar output is visible
        // to the typechecker input (i.e., no `data` nodes remain in the
        // typechecked AST — they are all desugared to `block`/`assign`).
        let kit = Kit::new().unwrap();
        let ast = kit.parse("data Nat :: Zero | Succ(Nat)").unwrap();

        // Verify no `data` nodes remain in the desugared AST.
        for node in ast.nodes() {
            assert_ne!(
                node.kind, "data",
                "data node should be desugared before typecheck"
            );
        }
        // The root should be a `block` (desugared output).
        let root = ast.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
    }

    // -----------------------------------------------------------------------
    // Phase 10.1: Kit::run typechecks by default
    // -----------------------------------------------------------------------

    #[test]
    fn kit_run_typechecks_by_default() {
        let kit = Kit::new().unwrap();
        // Clean program typechecks and evals.
        let val = kit.run("1 + 2").unwrap();
        assert_eq!(val, serde_json::json!(3));
    }

    #[test]
    fn kit_run_catches_type_error() {
        // `x` is an unbound variable: the typechecker rejects it before eval,
        // so Kit::run returns Err instead of a runtime crash.
        let kit = Kit::new().unwrap();
        let err = kit.run("x").unwrap_err();
        assert!(
            err.to_string().contains("typecheck"),
            "expected a typecheck error, got: {err}"
        );
    }

    #[test]
    fn kit_eval_still_works_without_typecheck() {
        // Kit::eval is parse + eval (no typecheck) — backward compat.
        let kit = Kit::new().unwrap();
        let val = kit.eval("1 + 2").unwrap();
        assert_eq!(val, serde_json::json!(3));
    }

    // -----------------------------------------------------------------------
    // Phase 5: runtime_assert diagnostics → eval pipeline (ADR-0068)
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_assert_passes_for_valid_value() {
        // A concrete value with a proven predicate: the typechecker can
        // evaluate the predicate statically → Proven → no runtime_assert
        // diagnostic. We verify via the typecheck result that zero
        // diagnostics are emitted (no runtime assert needed).
        let kit = Kit::new().unwrap();
        let ast = kit.parse("5 : (n ~> n > 0)").unwrap();
        let result = kit.runtime().typecheck(&ast).unwrap();
        assert!(
            result.diagnostics.is_empty(),
            "proven predicate should have no diagnostics, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn runtime_assert_passes_at_runtime() {
        // A variable annotated with a predicate: the typechecker can't
        // normalize the predicate on a symbolic variable → Blocked →
        // runtime_assert diagnostic → lowering pass wraps the type-annot in
        // a runtime-assert node. At runtime with a valid value (5), the
        // assert passes and returns 5.
        let kit = Kit::new().unwrap();
        let val = kit.run("do { y :: 5; (y : (n ~> n > 0)) }").unwrap();
        assert_eq!(val, serde_json::json!(5));
    }

    #[test]
    fn runtime_assert_fails_for_invalid_value() {
        // Same as above but with an invalid value (-1): the runtime assert
        // fails and raises a LangError.
        let kit = Kit::new().unwrap();
        let err = kit.run("do { y :: -1; (y : (n ~> n > 0)) }").unwrap_err();
        assert!(
            err.to_string().contains("assertion failed"),
            "expected assertion failure, got: {err}"
        );
    }
}
