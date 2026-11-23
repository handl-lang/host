//! Shared kit substrate — the pieces every `kit-*` crate composes on.
//!
//! Split out of `handl-kit-canonical` (now `handl-kit-prelude`) so each kit
//! is independently composable:
//!
//! - [`register_modules`] adds a kit's `(name → ModuleCtor)` rows to the
//!   process-wide [`MODULE_TABLE`]. Each `kit-*` crate exposes a
//!   `register()` that inserts its own rows; a host links only the kits it
//!   wants.
//! - [`KitResolver`] resolves module names through the table — it is the
//!   shared dependency-resolution service every `.hnl` module builder uses
//!   in place of the old monolithic `StdlibResolver`.
//! - [`build_registry`] materializes a [`ModuleRegistry`] over a selected
//!   subset of the table, sharing the process-wide module caches so a
//!   module built as a dependency and the same module registered at
//!   kit-assembly time share one `Arc` and one per-thread runtime cache.
//!
//! The caches and the table are singletons *here* on purpose: splitting
//! them per-kit would regress the `str-handl` per-thread-runtime fix —
//! every resolver and registry builder must reach the same instances.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, RwLock};

use handl_language::Value;
use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlModule, ModuleEnv, ModuleResolver, OpRegistry};
use handl_plugin_module::ModuleRegistry;

// ---------------------------------------------------------------------------
// Module table — name → constructor, populated by each kit's `register()`
// ---------------------------------------------------------------------------

/// What a kit registers for one module name.
///
/// `.hnl`-backed modules are [`ModuleEntry::Handl`] — they are additionally
/// tracked in [`HandlModules`] so a host can build handl-native
/// lint/rewrite adapters from them. Pure host `Module` impls are
/// [`ModuleEntry::Native`].
pub enum ModuleEntry {
    /// A `.hnl`-backed module (owned — the cache/registry takes the `Arc`).
    Handl(HandlModule),
    /// A native Rust [`Module`] impl.
    Native(Arc<dyn Module + Send + Sync>),
}

/// Constructor for one module.
///
/// `resolver` is the shared [`KitResolver`] so builders can materialize
/// dependencies through the table (`stdlib::module_with("x", resolver)`).
/// `eval_name` is the kit's configured eval engine (`"tree-walk"` when the
/// constructor runs through [`KitResolver`] outside a configured runtime).
pub type ModuleCtor = fn(&KitResolver, &'static str) -> ModuleEntry;

/// The process-wide module table every kit registers into.
///
/// A `Vec` (not a `HashMap`) because **insertion order is semantic**:
/// [`build_registry`] materializes the eval-time [`ModuleRegistry`] in this
/// order, and `ModuleLookup::lookup`'s flat-name pass returns the first
/// module exposing a given local function name. Ordering by registration
/// keeps an eponymous module (e.g. `sym`'s `sym`) ahead of later kits whose
/// modules happen to export a same-named local (e.g. `ffi`'s `sym`) —
/// the hand-ordered registration the canonical kit had before the
/// table-driven refactor.
static MODULE_TABLE: OnceLock<RwLock<Vec<(&'static str, ModuleCtor)>>> = OnceLock::new();

fn module_table() -> &'static RwLock<Vec<(&'static str, ModuleCtor)>> {
    MODULE_TABLE.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register a kit's module constructors into the shared table.
///
/// Last registration wins — a host that links two kits providing the same
/// name gets the one that registered last (the intended override seam); the
/// entry keeps its original position in the table order. Each `kit-*` crate
/// calls this from its own `register()`, which is expected to be
/// `Once`-protected by the caller.
pub fn register_modules(entries: &[(&'static str, ModuleCtor)]) {
    let mut table = module_table().write().expect("module table");
    for &(name, ctor) in entries {
        if let Some(slot) = table.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = ctor;
        } else {
            table.push((name, ctor));
        }
    }
}

/// The names currently in the table — diagnostic/introspection surface.
pub fn registered_module_names() -> Vec<String> {
    let mut names: Vec<String> = module_table()
        .read()
        .expect("module table")
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();
    names.sort_unstable();
    names
}

// ---------------------------------------------------------------------------
// Shared module caches
// ---------------------------------------------------------------------------

/// Process-wide cache for expensive pure-Handl module instances.
///
/// Every resolver and registry builder reaches the same `HandlModule` (and
/// therefore the same per-thread `Runtime`/`AST` caches) through
/// [`get_module`]. This is the fix for `str-handl` being rebuilt on every
/// `str.*` call, and it keeps `dedent`'s relative imports performant.
static MODULE_CACHE: OnceLock<RwLock<HashMap<String, Arc<dyn Module + Send + Sync>>>> =
    OnceLock::new();

/// Get a module from the cache or build it once and store it.
pub fn get_module(
    name: &str,
    builder: impl FnOnce() -> Arc<dyn Module + Send + Sync>,
) -> Arc<dyn Module + Send + Sync> {
    let cache = MODULE_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(m) = cache.read().expect("module cache").get(name) {
        return Arc::clone(m);
    }
    let module = builder();
    cache
        .write()
        .expect("module cache")
        .insert(name.to_string(), Arc::clone(&module));
    module
}

/// Cache of built-in `.hnl` [`HandlModule`]s keyed by their registered
/// name. Shared with [`KitResolver`] so a `.hnl` module built as a
/// dependency and the same module registered at kit-assembly time share
/// one [`Arc`] and one per-thread runtime cache.
static HANDL_MODULE_CACHE: OnceLock<RwLock<HashMap<String, Arc<HandlModule>>>> = OnceLock::new();

/// Get a `.hnl` [`HandlModule`] from the cache or build it once.
///
/// The builder runs *outside* the lock: `HandlModule::new`'s
/// `analyze_source` can run a nested eval whose module resolution
/// re-enters this cache — holding the lock across `builder()` would
/// deadlock on the same thread.
pub fn get_handl_module(name: &str, builder: impl FnOnce() -> HandlModule) -> Arc<HandlModule> {
    let cache = HANDL_MODULE_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(m) = cache.read().expect("handl module cache").get(name) {
        return Arc::clone(m);
    }
    let module = Arc::new(builder());
    cache
        .write()
        .expect("handl module cache")
        .insert(name.to_string(), Arc::clone(&module));
    module
}

/// Cache of env-instantiated `.hnl` [`HandlModule`]s keyed by
/// `(registered name, env_id)` — the env-aware sibling of
/// [`HANDL_MODULE_CACHE`] (ADR-0156 follow-up).
///
/// The name-keyed singleton above still maps one name to one module; this
/// table is additive, so one name can mean several envs — one entry per
/// `(name, env_id)`.
static HANDL_MODULE_ENV_CACHE: OnceLock<RwLock<HashMap<(String, usize), Arc<HandlModule>>>> =
    OnceLock::new();

/// Get a `.hnl` [`HandlModule`] instantiated under `env` from the cache or
/// build it once (ADR-0156 follow-up).
///
/// Env-aware sibling of [`get_handl_module`]: the *base* still comes from
/// the shared name-keyed cache (the source analysis runs once per name and
/// is reused by every env), and the returned instance is
/// [`HandlModule::instantiate`]d under `env` — the env replaces the base's
/// dependency binding wholesale — then cached by `(name, env_id)`. Clones
/// of one env share a single instance; distinct envs (distinct
/// [`ModuleEnv::env_id`]s) get distinct `module_id`s.
///
/// The existing single-env [`get_handl_module`] is unchanged. The
/// instantiate runs *outside* the lock, matching [`get_handl_module`]'s
/// re-entrancy note.
pub fn get_handl_module_with_env(
    name: &str,
    env: &ModuleEnv,
    builder: impl FnOnce() -> HandlModule,
) -> Arc<HandlModule> {
    let key = (name.to_string(), env.env_id());
    let cache = HANDL_MODULE_ENV_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(m) = cache.read().expect("handl module env cache").get(&key) {
        return Arc::clone(m);
    }
    let base = get_handl_module(name, builder);
    let module = Arc::new(base.instantiate(env.clone()));
    cache
        .write()
        .expect("handl module env cache")
        .insert(key, Arc::clone(&module));
    module
}

/// Coerce an `Arc<HandlModule>` to `Arc<dyn Module + Send + Sync>`.
pub fn coerce_handl(m: Arc<HandlModule>) -> Arc<dyn Module + Send + Sync> {
    m as Arc<dyn Module + Send + Sync>
}

// ---------------------------------------------------------------------------
// KitResolver — the shared name → module resolver
// ---------------------------------------------------------------------------

/// Resolver that maps module names to their [`Module`] impls through the
/// shared [`MODULE_TABLE`], so `.hnl` files can declare dependencies via
/// `from 'str' import * as 'str'` instead of hardcoding them in Rust.
///
/// Modules are built lazily through the shared caches; a name resolves
/// only if some linked kit registered it.
#[derive(Clone, Copy)]
pub struct KitResolver;

impl ModuleResolver for KitResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        let ctor = *module_table()
            .read()
            .expect("module table")
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, c)| c)?;
        match ctor(self, "tree-walk") {
            ModuleEntry::Handl(module) => {
                let arc = get_handl_module(name, move || module);
                Some(coerce_handl(arc))
            }
            ModuleEntry::Native(module) => Some(get_module(name, move || module)),
        }
    }
}

impl KitResolver {
    /// Resolve `name` instantiated under `env` (ADR-0156 follow-up).
    ///
    /// Env-aware sibling of [`ModuleResolver::resolve`]: `.hnl`-backed
    /// entries are built through the shared name-keyed base cache once
    /// per name, [`HandlModule::instantiate`]d under `env`, and cached by
    /// `(name, env_id)` — so one name can mean several envs (stub/mock /
    /// A-B dep surfaces) without a parallel module hierarchy.
    ///
    /// Native entries carry no environment — a host `Module` impl is not
    /// env-parameterized — so they resolve through the unchanged
    /// single-name [`get_module`], shared across envs.
    pub fn resolve_with_env(
        &self,
        name: &str,
        env: &ModuleEnv,
    ) -> Option<Arc<dyn Module + Send + Sync>> {
        let ctor = *module_table()
            .read()
            .expect("module table")
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, c)| c)?;
        match ctor(self, "tree-walk") {
            ModuleEntry::Handl(module) => {
                let arc = get_handl_module_with_env(name, env, move || module);
                Some(coerce_handl(arc))
            }
            ModuleEntry::Native(module) => Some(get_module(name, move || module)),
        }
    }
}

// ---------------------------------------------------------------------------
// Registry builder — table → ModuleRegistry over a selected subset
// ---------------------------------------------------------------------------

/// The `.hnl`-backed modules registered in a kit, tracked so a host can
/// build [`handl_plugin_ext_lint::HandlNativeLint`] /
/// [`handl_plugin_lib_handl::HandlNativeRewrite`] adapters from them
/// (ADR-0097 / ADR-0094).
pub struct HandlModules {
    modules: HashMap<String, Arc<HandlModule>>,
}

impl HandlModules {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, module: Arc<HandlModule>) {
        self.modules.insert(name.to_string(), module);
    }

    /// Build a [`handl_plugin_ext_lint::HandlNativeLint`] that resolves
    /// `handl:` lint rules against the registered [`HandlModule`]s.
    pub fn lint_adapter(&self) -> handl_plugin_ext_lint::HandlNativeLint {
        let mut adapter = handl_plugin_ext_lint::HandlNativeLint::new();
        for module in self.modules.values() {
            adapter = adapter.with_module_arc(Arc::clone(module));
        }
        adapter
    }

    /// Build a [`handl_plugin_lib_handl::HandlNativeRewrite`] that resolves
    /// `handl:` transform rules against the registered [`HandlModule`]s.
    pub fn rewrite_adapter(&self) -> handl_plugin_lib_handl::HandlNativeRewrite {
        let mut adapter = handl_plugin_lib_handl::HandlNativeRewrite::new();
        for module in self.modules.values() {
            adapter = adapter.with_module_arc(Arc::clone(module));
        }
        adapter
    }
}

/// The pair returned by [`build_registry`]: the eval-time
/// [`ModuleRegistry`] plus the [`HandlModules`] tracker that lets a host
/// build handl-native adapters from the same `.hnl` modules.
pub struct KitModules {
    pub registry: ModuleRegistry,
    pub handl_modules: HandlModules,
}

/// Register a `.hnl`-backed module in both the eval registry and the
/// handl-modules tracker, sharing the cached [`HandlModule`] built by
/// [`get_handl_module`].
fn register_handl(
    registry: &mut ModuleRegistry,
    handl_modules: &mut HandlModules,
    name: &str,
    builder: impl FnOnce() -> HandlModule,
) {
    let handl_arc = get_handl_module(name, builder);
    handl_modules.register(name, Arc::clone(&handl_arc));
    registry.register_arc(coerce_handl(handl_arc));
}

/// Materialize a [`ModuleRegistry`] over the registered module table.
///
/// `selected` narrows the table (a kit manifest's `base_modules` filtered
/// by any caller subset); `None` builds every registered module.
/// `eval_name` is the eval engine `.hnl`-backed modules are configured
/// with (`"tree-walk"` / `"bytecode"` / `"cranelift"`).
///
/// Built-in `.hnl` modules are prepared with the bootstrap transforms;
/// routing them through the process-wide self-hosted loader would recurse
/// into loader construction and make repeated kit construction hang.
pub fn build_registry(selected: Option<&HashSet<String>>, eval_name: &'static str) -> KitModules {
    handl_plugin_lib_handl::with_loader_disabled(|| {
        let mut stdlib_registry = ModuleRegistry::new();
        let mut handl_modules = HandlModules::new();

        // Registration order is deterministic and semantic (see the
        // `MODULE_TABLE` docs): flat-name module lookup returns the first
        // module exposing a local function name, so kit registration order
        // decides precedence for colliding local names (`sym` before `ffi`).
        let entries: Vec<(&'static str, ModuleCtor)> =
            module_table().read().expect("module table").clone();

        let want = |name: &str| selected.map(|s| s.contains(name)).unwrap_or(true);
        let resolver = KitResolver;

        for (name, ctor) in entries {
            if !want(name) {
                continue;
            }
            match ctor(&resolver, eval_name) {
                ModuleEntry::Handl(module) => {
                    register_handl(&mut stdlib_registry, &mut handl_modules, name, move || {
                        module
                    });
                }
                ModuleEntry::Native(module) => {
                    stdlib_registry.register_arc(module);
                }
            }
        }

        KitModules {
            registry: stdlib_registry,
            handl_modules,
        }
    })
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
//
// This module is part of the bootstrap floor: `ffi`, the test framework,
// and `dedent`'s relative imports all build it directly, so it lives in
// core rather than in any one kit.

/// The handl source implementing the `type` contract
/// (`handl-packages/type/type.hnl`) as pure handl over the `type.of` atom.
/// Owned by `handl-lib-type-handl` (ADR-0128 ".hnl ownership"); consumed
/// here as a `pub const` so the kit builds without a sibling checkout.
const TYPE_HANDL_HNL: &str =
    include_str!(concat!(env!("OUT_DIR"), "/lib-type-handl/src/index.hnl"));

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
    "Int",
    "Number",
    "Nat",
    "Obj",
    "String",
    "Array",
    "Bool",
    "Value",
    "Type",
    "Unit",
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
/// - `type.Int` / `type.Number` / `type.Float` / `type.Nat` / `type.Str` /
///   `type.String` / `type.Bool` / `type.Null` / `type.Obj` / `type.Array` /
///   `type.Value` / `type.Type` / `type.Grade` / `type.Unit` — the builtin
///   type constants. Since WS-E (ADR-0154 §1/§2) these mint *seeded
///   nominal* values `{"__sym__": {handle, name, parent, type}}` — the
///   deterministic handle is a content hash of the `type.<Name>` seed, the
///   `parent` link encodes `<:` (`Nat <: Int <: Number <: Value`), the
///   `type` link encodes membership (`Int.ty = Type`). `__sym__` is a
///   reserved key a handl object literal cannot produce (the eval
///   `object` op rejects it, ADR-0055/ADR-0054), so minting is a host
///   capability — the same seam `tag.tag`/`tag.construct` occupy. The
///   `type-handl` record fields call these nullary ops rather than writing
///   the forbidden literal.
/// - `type.is_nominal` / `type.instanceof` / `type.ty` — the ADR-0154 §2
///   link readers (nominal predicate; membership-via-type-link +
///   subtyping-via-parent-chain; the `x : T` type link) — delegating to
///   `handl-value`'s `is_nominal`/`instance_of`/`type_link`, the same
///   implementations behind `instanceof`.
const TYPE_OPS: &[&str] = &[
    "type.of",
    "type.to_string",
    "type.to_number",
    "type.coalesce",
    "type.is_nominal",
    "type.instanceof",
    "type.ty",
    "type.Int",
    "type.Number",
    "type.Float",
    "type.Nat",
    "type.Str",
    "type.String",
    "type.Bool",
    "type.Null",
    "type.Obj",
    "type.Array",
    "type.Value",
    "type.Type",
    "type.Grade",
    "type.Unit",
];

/// `type.of(v)` — return the kind tag of a value, or its nominal `type`
/// link when it carries one (ADR-0154 §2).
///
/// A value with an explicit type link reports that link rather than the
/// kind tag: a `{"__ctor__": …, "type": F(i⃗)}` datum yields its `type`
/// field (the former application), and a nominal `{"__sym__": …}` yields
/// its `ty` link (`type.Int` → `type.Type`). Unlinked values — literals,
/// arrays, plain objects, arrows, `__tag__` data — keep the kind-tag
/// read (the `__tag__`→nominal bridge is deleted; a tagged datum is a
/// plain `"object"` here, as it is to `type.ty`/`instanceof`).
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
        // Prefer the explicit `type` link — the same arms of
        // `handl_value::type_link` that read *declared* membership,
        // excluding the literal/kind defaults and the `__tag__` bridge
        // (those keep the kind-tag fallback below).
        let tv = handl_value::TaggedValue::from(v.clone());
        let linked = match &tv {
            handl_value::TaggedValue::Sym(s) => s
                .ty()
                .map(|t| handl_value::TaggedValue::Sym(std::rc::Rc::new(t.clone()))),
            handl_value::TaggedValue::Obj(m) => m.get("type").cloned(),
            _ => None,
        };
        if let Some(t) = linked {
            return Ok(handl_value::tagged_to_value(&t));
        }
        Ok(Value::from(type_tag_json(v).to_string()))
    }
}

/// `type.<Name>()` — mint a builtin type *nominal* `{"__sym__":
/// {handle, name, parent, type}}` (ADR-0154 §1/§2).
///
/// The mint is deterministic: the handle is a content hash of the
/// `type.<Name>` seed, so `type.Int()` in any module, artifact, or
/// process yields the *same* nominal — equality is identity, not
/// structure. `parent`/`type` links come from `handl-value`'s
/// `BUILTIN_TYPE_GRAPH` (`Nat <: Int <: Number <: Value`; every type's
/// `ty` is `Type`).
///
/// Minting is a host capability: `__sym__` is a reserved key that no
/// handl object literal may produce (the eval `object` op rejects it —
/// ADR-0055 §"Negative — A reserved key", ADR-0054). The `type-handl`
/// module previously wrote `__tag__` literals inline, which made the
/// module record unevaluable; the record fields call these nullary ops
/// instead. (The legacy `{"__tag__": …}` type values are superseded by
/// these nominals — the `__tag__` bridge in `type.ty`/`instanceof` is
/// deleted; a tagged datum is a plain `type.Obj` member.)
#[derive(Clone, Copy, Debug)]
pub struct TypeConst {
    /// The `type.*` seed the minted nominal carries (e.g. `"type.Int"`).
    seed: &'static str,
}

impl Function for TypeConst {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                0,
                args.len()
            )));
        }
        Ok(handl_value::tagged_to_value(&handl_value::builtin_type(
            self.seed,
        )))
    }
}

/// `type.is_nominal(v)` — whether `v` is a nominal (`{"__sym__"}`)
/// value. The `type.is`/`kind_is` dispatch reads it: a nominal kind
/// argument means "membership" (instanceof), anything else is a kind
/// spelling.
#[derive(Clone, Copy, Debug)]
pub struct TypeIsNominal;

impl Function for TypeIsNominal {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1,
            args.len()
        )))?;
        Ok(Value::Bool(handl_value::is_nominal(
            &handl_value::TaggedValue::from(v.clone()),
        )))
    }
}

/// `type.instanceof(v, nominal)` — `v : T` membership per ADR-0154 §2:
/// `v`'s `type`-link head, then the `parent` (`<:`) walk. Same
/// implementation as `instanceof` (`handl_value::instance_of`) — the
/// nominal argument resolves through `nominal_head` (sym, former
/// application, universe encoding).
#[derive(Clone, Copy, Debug)]
pub struct TypeInstanceof;

impl Function for TypeInstanceof {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len()
        )))?;
        let n = args.get(1).ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len()
        )))?;
        let nv = handl_value::TaggedValue::from(n.clone());
        if handl_value::nominal_head(&nv).is_none() {
            return Err(EvalError::TypeMismatch(
                "type.instanceof: second argument must be a nominal object (see ADR-0106 §2)"
                    .to_string(),
            ));
        }
        Ok(Value::Bool(handl_value::instance_of(
            &handl_value::TaggedValue::from(v.clone()),
            &nv,
        )))
    }
}

/// `type.ty(v)` — the `type` link of `v` (`x : T`; ADR-0154 §2): a
/// nominal's `ty` link, a ctor object's `"type"` field, a literal's
/// default kind nominal (`1 → type.Nat`, `-1 → type.Int`,
/// `1.5 → type.Float`), `null` when no link resolves. (The `__tag__`
/// bridge is deleted — a tagged datum resolves to `type.Obj`.)
#[derive(Clone, Copy, Debug)]
pub struct TypeTy;

impl Function for TypeTy {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let v = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1,
            args.len()
        )))?;
        Ok(
            handl_value::type_link(&handl_value::TaggedValue::from(v.clone()))
                .map(|t| handl_value::tagged_to_value(&t))
                .unwrap_or(Value::Null),
        )
    }
}

/// `(op name, seed)` pairs for every builtin type constant — the field
/// names `type-handl.hnl` re-exports. `Str` and `String` share the
/// `"type.Str"` seed — two spellings of the same nominal, and `type.Str()
/// = type.String()` by handle identity.
const TYPE_CONSTS: &[(&str, &str)] = &[
    ("type.Int", "type.Int"),
    ("type.Number", "type.Number"),
    ("type.Float", "type.Float"),
    ("type.Nat", "type.Nat"),
    ("type.Str", "type.Str"),
    ("type.String", "type.Str"),
    ("type.Bool", "type.Bool"),
    ("type.Null", "type.Null"),
    ("type.Obj", "type.Obj"),
    ("type.Array", "type.Array"),
    ("type.Value", "type.Value"),
    ("type.Type", "type.Type"),
    // `lib-grade`'s `Grade` kind for effect-graded arrows — consumers
    // annotate port types `export Port<T>: Grade`.
    ("type.Grade", "type.Grade"),
    ("type.Unit", "type.Unit"),
];

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
    // `to_string`/`to_number`/`coalesce` are kernel `handl_ops::Op`s since
    // the `handl-lib-type` migration — register them in a kernel registry
    // under the `type.*` names and import.
    let mut kernel = handl_ops::OpRegistry::new();
    kernel.register("type.to_string", std::sync::Arc::new(ToString));
    kernel.register("type.to_number", std::sync::Arc::new(ToNumber));
    kernel.register("type.coalesce", std::sync::Arc::new(Coalesce));
    registry.import_kernel(&kernel);
    // ADR-0154 §2 link readers — the same `handl-value` implementations
    // behind `instanceof`.
    registry.register("type.is_nominal", std::sync::Arc::new(TypeIsNominal));
    registry.register("type.instanceof", std::sync::Arc::new(TypeInstanceof));
    registry.register("type.ty", std::sync::Arc::new(TypeTy));
    // Builtin type constants — host-minted seeded nominals
    // (`__sym__` reserved key; the `type-handl` record fields call these
    // ops).
    for &(op_name, seed) in TYPE_CONSTS {
        registry.register(op_name, std::sync::Arc::new(TypeConst { seed }));
    }
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
pub fn build_type_module() -> HandlModule {
    HandlModule::new("type", TYPE_HANDL_HNL, TYPE_HANDL_FUNCS)
        .with_dependency(type_ops_module())
        .with_ops(TYPE_OPS, std::sync::Arc::new(type_op_registry()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ADR-0156 follow-up: env-aware module keys ─────────────────────
    //
    // Same name, two envs → two instances (distinct `module_id`, each
    // env's answers) — the env-keyed caches above make one name mean
    // several envs.

    /// Install the canonical default language once per test binary (the
    /// library links no language data; tests assemble it from the dev-dep
    /// data crate — the same pattern as `plugin-lib-handl`'s
    /// `multi_env.rs`).
    fn ensure_lang() {
        if handl_plugin_lib_handl::default_language().is_none() {
            let _ = handl_plugin_lib_handl::install_default_language(
                handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
            );
        }
    }

    const ENV_SRC: &str =
        "from \"dep\" import * as dep\nexport ask := (x) ~> dep.who() + \":\" + x\n";

    /// A `dep` stub whose `who` answers a marker string.
    struct EnvStubWho(&'static str);

    impl Function for EnvStubWho {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(serde_json::json!(self.0))
        }
    }

    struct EnvStubDep {
        who: EnvStubWho,
    }

    impl EnvStubDep {
        fn new(marker: &'static str) -> Self {
            Self {
                who: EnvStubWho(marker),
            }
        }
    }

    impl Module for EnvStubDep {
        fn name(&self) -> &str {
            "dep"
        }

        fn function(&self, name: &str) -> Option<&dyn Function> {
            (name == "who").then_some(&self.who as &dyn Function)
        }

        fn functions(&self) -> Vec<(&str, &dyn Function)> {
            vec![("who", &self.who)]
        }
    }

    fn call_ask(m: &HandlModule, arg: &str) -> Value {
        m.function("ask")
            .expect("ask bound")
            .call(&[serde_json::json!(arg)], &mut EvalContext::new())
            .expect("ask evaluates")
    }

    /// [`get_handl_module_with_env`]: one name, several envs — a fresh
    /// instance per env (distinct `module_id`, that env's answers), one
    /// shared instance per env *clone*.
    #[test]
    fn env_keyed_handl_module_cache_serves_one_instance_per_env() {
        ensure_lang();
        let env_a = ModuleEnv::new().with_dependency(EnvStubDep::new("alpha"));
        let env_b = ModuleEnv::new().with_dependency(EnvStubDep::new("beta"));

        let a = get_handl_module_with_env("env_probe", &env_a, || {
            HandlModule::new("env_probe", ENV_SRC, &["ask"])
        });
        let b = get_handl_module_with_env("env_probe", &env_b, || {
            HandlModule::new("env_probe", ENV_SRC, &["ask"])
        });

        assert_ne!(a.module_id(), b.module_id(), "one instance per env");
        assert_eq!(call_ask(&a, "x"), serde_json::json!("alpha:x"));
        assert_eq!(call_ask(&b, "x"), serde_json::json!("beta:x"));
        // Re-dispatch through `a` after `b` ran: distinct module_ids keep
        // the per-thread runtime caches apart.
        assert_eq!(call_ask(&a, "y"), serde_json::json!("alpha:y"));
        // A clone is the same env → the same cached instance.
        let a_again = get_handl_module_with_env("env_probe", &env_a.clone(), || {
            HandlModule::new("env_probe", ENV_SRC, &["ask"])
        });
        assert!(Arc::ptr_eq(&a, &a_again), "env clones share the instance");
    }

    fn env_probe_ctor(_resolver: &KitResolver, _eval_name: &'static str) -> ModuleEntry {
        ModuleEntry::Handl(HandlModule::new("env_probe_res", ENV_SRC, &["ask"]))
    }

    /// [`KitResolver::resolve_with_env`]: a registered table name resolves
    /// to a distinct instance per env and to one shared instance per env
    /// clone; the single-env `resolve` keeps serving the shared base.
    #[test]
    fn kit_resolver_resolve_with_env_instantiates_per_env() {
        ensure_lang();
        register_modules(&[("env_probe_res", env_probe_ctor)]);

        let resolver = KitResolver;
        let env_a = ModuleEnv::new().with_dependency(EnvStubDep::new("alpha"));
        let env_b = ModuleEnv::new().with_dependency(EnvStubDep::new("beta"));

        let a = resolver
            .resolve_with_env("env_probe_res", &env_a)
            .expect("env a resolves");
        let b = resolver
            .resolve_with_env("env_probe_res", &env_b)
            .expect("env b resolves");
        let a_again = resolver
            .resolve_with_env("env_probe_res", &env_a.clone())
            .expect("env a clone resolves");

        assert!(Arc::ptr_eq(&a, &a_again), "env clones share the instance");
        assert!(!Arc::ptr_eq(&a, &b), "distinct envs are distinct instances");

        let ask = |m: &Arc<dyn Module + Send + Sync>, arg: &str| {
            m.function("ask")
                .expect("ask bound")
                .call(&[serde_json::json!(arg)], &mut EvalContext::new())
                .expect("ask evaluates")
        };
        assert_eq!(ask(&a, "x"), serde_json::json!("alpha:x"));
        assert_eq!(ask(&b, "x"), serde_json::json!("beta:x"));

        // The single-env path is unchanged: the same name still resolves
        // to the shared base instance, not an env instance.
        let plain = resolver.resolve("env_probe_res").expect("plain resolve");
        assert!(!Arc::ptr_eq(&plain, &a));
        assert!(!Arc::ptr_eq(&plain, &b));
    }

    /// The `type.<Name>` const ops mint the builtin type *nominal* —
    /// `{"__sym__": {handle, name, parent, type}}` (ADR-0154 §1/§2): the
    /// deterministic seeded identity carrying the `<:` `parent` and the
    /// `type.Type` membership link, in place of the superseded
    /// `{"__tag__": t, "__registry__": "type", "kind": "primitive"}`
    /// encoding. Host minting stays the only legal way to produce type
    /// values (`__sym__`/`__tag__` are reserved keys, ADR-0055/ADR-0054).
    #[test]
    fn type_const_ops_mint_builtin_type_values() {
        let module = build_type_module();
        for &(op_name, seed) in TYPE_CONSTS {
            let local = op_name.rsplit('.').next().unwrap();
            let f = module
                .function(local)
                .unwrap_or_else(|| panic!("type module must expose {local}"));
            let mut ctx = EvalContext::new();
            let v = f
                .call(&[], &mut ctx)
                .unwrap_or_else(|e| panic!("{op_name} must mint: {e}"));
            let sym = v
                .get("__sym__")
                .unwrap_or_else(|| panic!("{op_name} must mint a nominal, got {v}"));
            let short = seed.rsplit('.').next().unwrap();
            assert_eq!(
                sym.get("name"),
                Some(&serde_json::json!(short)),
                "{op_name} nominal name"
            );
            // Membership: every builtin type's `type` link is `Type`.
            assert_eq!(
                sym.get("type")
                    .and_then(|t| t.get("__sym__"))
                    .and_then(|s| s.get("name")),
                Some(&serde_json::json!("Type")),
                "{op_name} membership link"
            );
        }
    }

    /// The `type` module exposes both the pure-handl fields (resolved
    /// through the `type-handl` record — `type.is_array`, `type.is`, …)
    /// and the native const ops. Regression cover for the record-eval
    /// failure that broke `type.is_array` ("object: key '__tag__' is
    /// reserved"): the `.hnl` record must not carry the reserved keys —
    /// `type-handl.hnl` calls `type.<Name>()` ops that mint them, and this
    /// test pins the op surface those calls resolve to.
    #[test]
    fn type_module_exposes_handl_fields_and_const_ops() {
        let module = build_type_module();
        for name in [
            "is_array", "is_null", "is", "must", "may", "not", "when", "to_bool",
        ] {
            assert!(module.function(name).is_some(), "missing {name}");
        }
        for &(op_name, _) in TYPE_CONSTS {
            let local = op_name.rsplit('.').next().unwrap();
            assert!(module.function(local).is_some(), "missing {local}");
        }
    }
}
