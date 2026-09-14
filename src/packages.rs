//! The stdlib **package table** — every handl-backed library is a row of
//! data, not a crate.
//!
//! A [`ModuleSpec`] describes one module: its name, its `.hnl` source
//! (`include_str!`ed at the call site), the record-field function names,
//! the op names to dispatch via an [`OpRegistry`], and the names of
//! dependency modules (resolved through a [`ModuleResolver`]).
//!
//! A [`PackageSpec`] groups modules under one library-plugin name — the
//! key used by kit manifests (`modules:` entries) and
//! `handl_kit_data::LibraryRegistry`.
//!
//! [`module`] / [`module_ref`] build a [`HandlModule`] for a module name;
//! [`library`] builds a `Box<dyn Plugin>` for a package name.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    Function, Module, ModuleLookup, PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::{HandlModule, ModuleResolver, OpRegistry, parse_cap_declarations};
use handl_plugin_module::{ModuleRule, ModuleRuntime};

use crate::ops;

// ---------------------------------------------------------------------------
// Spec types
// ---------------------------------------------------------------------------

/// One row of the module table: everything needed to build a
/// [`HandlModule`] for a `.hnl` package.
pub struct ModuleSpec {
    /// The module namespace (e.g. `"bool"`, `"math"`, `"typeof"`).
    pub name: &'static str,
    /// The `.hnl` source (already `include_str!`ed).
    pub hnl: &'static str,
    /// Record-field function names exposed by the source.
    pub funcs: &'static [&'static str],
    /// Op names dispatched to the `registry` builder (e.g. `"math.add"`).
    /// Local names are the segment after the last `.`.
    pub ops: &'static [&'static str],
    /// Builder for the op registry, or `None` for pure-handl modules.
    pub registry: Option<fn() -> OpRegistry>,
}

/// One row of the package table: a named library contributing one or more
/// modules (the replacement for per-crate `*LibraryPlugin` shells).
pub struct PackageSpec {
    /// The library-registry key used by kit manifests (e.g. `"utils"`).
    pub lib_name: &'static str,
    /// The stable plugin id (e.g. `"lib.utils"`).
    pub plugin_id: &'static str,
    /// The plugin display name (e.g. `"lib-utils"`).
    pub plugin_name: &'static str,
    /// The `.hnl` source declaring the package interface — used for `cap`
    /// declarations surfaced via [`Plugin::capabilities`].
    pub hnl: &'static str,
    /// The modules this package contributes.
    pub modules: &'static [ModuleSpec],
}

// ---------------------------------------------------------------------------
// The module table
// ---------------------------------------------------------------------------

const CLOCK_GETTIME_MODULE: ModuleSpec = ModuleSpec {
    name: "clock_gettime",
    hnl: include_str!("../../../handl-packages/time-c/clock_gettime.hnl"),
    funcs: &["read_ns"],
    ops: &[],
    registry: None,
};

const ENTROPY_MODULE: ModuleSpec = ModuleSpec {
    name: "entropy",
    hnl: include_str!("../../../handl-packages/rand-c/entropy.hnl"),
    funcs: &["next_u32"],
    ops: &[],
    registry: None,
};

/// All table-built modules, keyed by module name.
pub static MODULES: &[ModuleSpec] = &[
    CLOCK_GETTIME_MODULE,
    ENTROPY_MODULE,
    // ── utils (former plugin-lib-utils) ────────────────────────────────
    ModuleSpec {
        name: "bool",
        hnl: include_str!("../../../handl-packages/utils/bool.hnl"),
        funcs: &["and", "or", "not", "xor", "nand", "nor", "implies", "eq"],
        ops: &[],
        registry: None,
    },
    ModuleSpec {
        name: "math",
        hnl: include_str!("../../../handl-packages/utils/math.hnl"),
        funcs: &[],
        ops: ops::math::MATH_OPS,
        registry: Some(ops::math::math_op_registry),
    },
    ModuleSpec {
        name: "str",
        hnl: include_str!("../../../handl-packages/utils/str.hnl"),
        funcs: &[],
        ops: ops::strs::STR_OPS,
        registry: Some(ops::strs::str_op_registry),
    },
    ModuleSpec {
        name: "combinators",
        hnl: include_str!("../../../handl-packages/utils/fn.hnl"),
        funcs: &[],
        ops: ops::combinators::COMBINATORS_OPS,
        registry: Some(ops::combinators::combinators_op_registry),
    },
    ModuleSpec {
        name: "transform",
        hnl: include_str!("../../../handl-packages/transform/transform.hnl"),
        funcs: &[],
        ops: ops::combinators::TRANSFORM_OPS,
        registry: Some(ops::combinators::transform_op_registry),
    },
    ModuleSpec {
        name: "typeof",
        hnl: include_str!("../../../handl-packages/utils/type.hnl"),
        funcs: &[],
        ops: ops::type_ops::TYPE_OPS,
        registry: Some(ops::type_ops::type_op_registry),
    },
    ModuleSpec {
        name: "option",
        hnl: include_str!("../../../handl-packages/option/index.hnl"),
        funcs: &["Option", "some", "none"],
        ops: &[],
        registry: None,
    },
    ModuleSpec {
        name: "mood",
        hnl: include_str!("../../../handl-packages/utils/mood.hnl"),
        funcs: &[],
        ops: ops::mood::MOOD_OPS,
        registry: Some(ops::mood::mood_op_registry),
    },
    ModuleSpec {
        name: "encoding",
        hnl: include_str!("../../../handl-packages/utils/encoding.hnl"),
        funcs: &[],
        ops: ops::encoding::ENCODING_OPS,
        registry: Some(ops::encoding::encoding_op_registry),
    },
    ModuleSpec {
        name: "parse",
        hnl: include_str!("../../../handl-packages/utils/parse.hnl"),
        funcs: &[],
        ops: ops::parse::PARSE_OPS,
        registry: Some(ops::parse::parse_op_registry),
    },
    ModuleSpec {
        name: "serialize",
        hnl: include_str!("../../../handl-packages/utils/serialize.hnl"),
        funcs: &[],
        ops: ops::serialize::SERIALIZE_OPS,
        registry: Some(ops::serialize::serialize_op_registry),
    },
    // ── codecs / formats (former plugin-lib-{base64,hex,json,toml,yaml}) ──
    ModuleSpec {
        name: "base64",
        hnl: include_str!("../../../handl-packages/base64/base64.hnl"),
        funcs: &[],
        ops: ops::base64::BASE64_OPS,
        registry: Some(ops::base64::base64_op_registry),
    },
    ModuleSpec {
        name: "hex",
        hnl: include_str!("../../../handl-packages/hex/hex.hnl"),
        funcs: &[],
        ops: ops::hex::HEX_OPS,
        registry: Some(ops::hex::hex_op_registry),
    },

    // ── url parsers (former plugin-lib-url-{rfc,whatwg}) ───────────────
    ModuleSpec {
        name: "url_rfc",
        hnl: include_str!("../../../handl-packages/url_rfc/url_rfc.hnl"),
        funcs: &[],
        ops: ops::url_rfc::URL_RFC_OPS,
        registry: Some(ops::url_rfc::url_rfc_op_registry),
    },
    ModuleSpec {
        name: "url_whatwg",
        hnl: include_str!("../../../handl-packages/url_whatwg/url_whatwg.hnl"),
        funcs: &[],
        ops: ops::url_whatwg::URL_WHATWG_OPS,
        registry: Some(ops::url_whatwg::url_whatwg_op_registry),
    },
    // ── cmp (former plugin-lib-cmp) ────────────────────────────────────
    ModuleSpec {
        name: "cmp",
        hnl: include_str!("../../../handl-packages/cmp/cmp.hnl"),
        funcs: &[],
        ops: ops::cmp::CMP_OPS,
        registry: Some(ops::cmp::cmp_op_registry),
    },
    // ── collections (former plugin-lib-collections) ────────────────────
    // Sequential containers: the whole non-nullary surface is pure handl
    // over plain arrays; `ops` holds only `new`/`from`/`to_array`
    // (`new` is nullary, `from` is a reserved word). `pop`/`peek`/
    // `dequeue`/`pop_*`/`peek_*` raise `collection is empty` via
    // `error.raise` — the `error` dep resolves through the table.
    ModuleSpec {
        name: "vec",
        hnl: include_str!("../../../handl-packages/collections/vec.hnl"),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::VEC_OPS,
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "list",
        hnl: include_str!("../../../handl-packages/collections/list/index.hnl"),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::LIST_OPS,
        registry: Some(ops::collections::collections_op_registry),
    },
    // `list` per-arrow modules (`collections/list/<name>.hnl`), mirroring
    // `array`'s per-arrow split. IMPORTANT: unlike `array`, each arrow is
    // registered under a `list_`-prefixed module name (e.g. `list_len`,
    // `list_pop`) because `array` already owns the bare names `len` /
    // `is_empty` / `pop` in this flat table — the global module name space
    // cannot hold both. Qualified references bind to the dependency's
    // registered module name (not an arbitrary alias), so `list/index.hnl`
    // imports from these prefixed names and re-exports under the bare
    // `list.*` fields, e.g. `from "list_len" import * as list_len` then
    // `len: list_len.len`, and so on.
    ModuleSpec { name: "list_len", hnl: include_str!("../../../handl-packages/collections/list/len.hnl"), funcs: &["len"], ops: &[], registry: None },
    ModuleSpec { name: "list_is_empty", hnl: include_str!("../../../handl-packages/collections/list/is_empty.hnl"), funcs: &["is_empty"], ops: &[], registry: None },
    ModuleSpec { name: "list_push", hnl: include_str!("../../../handl-packages/collections/list/push.hnl"), funcs: &["push"], ops: &[], registry: None },
    ModuleSpec { name: "list_peek", hnl: include_str!("../../../handl-packages/collections/list/peek.hnl"), funcs: &["peek"], ops: &[], registry: None },
    ModuleSpec { name: "list_pop", hnl: include_str!("../../../handl-packages/collections/list/pop.hnl"), funcs: &["pop"], ops: &[], registry: None },
    ModuleSpec {
        name: "queue",
        hnl: include_str!("../../../handl-packages/collections/queue.hnl"),
        funcs: &["len", "is_empty", "enqueue", "dequeue", "peek"],
        ops: ops::collections::QUEUE_OPS,
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "stack",
        hnl: include_str!("../../../handl-packages/collections/stack.hnl"),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::STACK_OPS,
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "deque",
        hnl: include_str!("../../../handl-packages/collections/deque.hnl"),
        funcs: &[
            "len",
            "is_empty",
            "push_back",
            "push_front",
            "pop_back",
            "pop_front",
            "peek_back",
            "peek_front",
        ],
        ops: ops::collections::DEQUE_OPS,
        registry: Some(ops::collections::collections_op_registry),
    },
    // `hash_map`/`hash_set` stay mostly native (see the module .hnl docs);
    // the field-enumeration surface (`keys`/`values`/`entries`) is now
    // pure handl over the `obj` introspection primitives.
    ModuleSpec {
        name: "hash_map",
        hnl: include_str!("../../../handl-packages/collections/hash_map.hnl"),
        funcs: &[
            "keys",
            "values",
            "entries",
            "len",
            "is_empty",
            "has",
            "has_key",
            "clone_deep",
        ],
        ops: ops::collections::HASH_MAP_OPS,
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "hash_set",
        hnl: include_str!("../../../handl-packages/collections/hash_set.hnl"),
        funcs: &[],
        ops: ops::collections::HASH_SET_OPS,
        registry: Some(ops::collections::collections_op_registry),
    },
    // The `array` package is loaded from `handl-packages/collections/array/index.hnl`
    // at runtime via `crate::loader::load_hnl_file` instead of being embedded here.
    // `hash_map` still depends on the leaf `len` and `contains` submodules.
    ModuleSpec { name: "len", hnl: include_str!("../../../handl-packages/collections/array/len.hnl"), funcs: &["len"], ops: &[], registry: None },
    ModuleSpec { name: "contains", hnl: include_str!("../../../handl-packages/collections/array/contains.hnl"), funcs: &["contains"], ops: &[], registry: None },
    // ── stats (former plugin-lib-stats) — pure handl ───────────────────
    ModuleSpec {
        name: "stats",
        hnl: include_str!("../../../handl-packages/stats/stats.hnl"),
        funcs: &["sum", "avg", "product", "min", "max", "median", "stddev"],
        ops: &[],
        registry: None,
    },
    // ── hello (former plugin-lib-hello) — pure handl demo greeting ────
    // `greet.hnl` is a bare arrow (`name ~> "Hello, " + name + "!"`), not
    // a record of arrows. `HandlModule` evaluates `(source).func(args)`,
    // which requires a record, so the bare arrow is wrapped into a
    // single-field record `{ greet: (<greet.hnl>) }` (comment and
    // declaration lines are stripped by the loader).
    ModuleSpec {
        name: "hello",
        hnl: concat!(
            "{ greet: (\n",
            include_str!("../../../handl-packages/hello/greet.hnl"),
            "\n) }"
        ),
        funcs: &["greet"],
        ops: &[],
        registry: None,
    },
    // ── result (former plugin-lib-result) — pure handl over `tag.*` ───
    // `result.ok`/`result.err` are `tag` constructor arrows: the
    // `tag ok = Value` / `tag err = Error` declarations expand to
    // `tag.tag("ok", {value: _0})` / `tag.tag("err", {value: _0})` calls,
    // so the `tag` module must resolve through the ambient
    // [`ModuleResolver`]. The default table resolver does not know
    // `"tag"` (it stays in `plugin-lib-tag`); the injected global table
    // lookup resolves it through the resolver passed to `module_with`
    // (e.g. kit-canonical's `StdlibResolver`).
    ModuleSpec {
        name: "result",
        hnl: include_str!("../../../handl-packages/result/index.hnl"),
        funcs: &["Result", "ok", "err"],
        ops: &[],
        registry: None,
    },
    // ── obj (record introspection) ────────────────────────────────────
    // `obj.keys`/`obj.values`/`obj.entries` are the object-field
    // enumeration primitives (gap #3); used by `hash_map` and other
    // record ops.
    ModuleSpec {
        name: "obj",
        hnl: include_str!("../../../handl-packages/obj/obj.hnl"),
        funcs: &[],
        ops: ops::obj::OBJ_OPS,
        registry: Some(ops::obj::obj_op_registry),
    },
    // ── error (ADR-0092 gap (a)) — the error-raising primitive ─────────
    // `error.raise` is an irreducible atom: aborting evaluation cannot be
    // expressed over the eval vocabulary, so the module is ops-only. Any
    // `.hnl` module that needs to fail loudly resolves `error` through the
    // global lookup (or registers the module in its ambient chain) and calls
    // `error.raise(msg)`.
    ModuleSpec {
        name: "error",
        hnl: include_str!("../../../handl-packages/error/error.hnl"),
        funcs: &[],
        ops: ops::error::ERROR_OPS,
        registry: Some(ops::error::error_op_registry),
    },
    // ── arrow_iter (former plugin-lib-arrow-iter) ──────────────────────
    // Declaration-only module: the `while`/`for`/`fix` derived forms are
    // `Term::Loop` descriptions built host-side (see
    // [`crate::arrow_iter`]); the composite `*_step` ops are interpreted
    // by the v2 engine, so the module exposes no functions or ops.
    ModuleSpec {
        name: "arrow_iter",
        hnl: include_str!("../../../handl-packages/arrow_iter/arrow_iter.hnl"),
        funcs: &[],
        ops: &[],
        registry: None,
    },
    // ── prim (former plugin-lib-prim) ──────────────────────────────────
    // Pure-handl record: `add`/`alloc`/`or` over builtins/`math`,
    // `compare` over `cmp.compare` to preserve exact total-ordering.
    ModuleSpec {
        name: "prim",
        hnl: include_str!("../../../handl-packages/prim-handl/prim-handl.hnl"),
        funcs: &["add", "alloc", "or", "compare"],
        ops: &[],
        registry: None,
    },
    // ── archive / archive_tar (former plugin-lib-archive, plugin-lib-archive-tar)
    // The `archive` module is caps-only (no callable ops — it's a trait
    // abstraction). The `tar` module provides `tar.pack` and `tar.unpack` via
    // the consolidated Rust `Function` impls in `ops::tar`.
    ModuleSpec {
        name: "archive",
        hnl: include_str!("../../../handl-packages/archive/archive.hnl"),
        funcs: &[],
        ops: &[],
        registry: None,
    },
    ModuleSpec {
        name: "tar",
        hnl: include_str!("../../../handl-packages/archive_tar/archive_tar.hnl"),
        funcs: &[],
        ops: ops::tar::TAR_OPS,
        registry: Some(ops::tar::tar_op_registry),
    },
];

/// The modules contributed by the `utils` aggregate package (matches the
/// former `UtilsLibraryPlugin::contribute` module list).
const UTILS_MODULES: &[&str] = &[
    "math",
    "str",
    "bool",
    "combinators",
    "transform",
    "typeof",
    "option",
    "mood",
    "encoding",
    "parse",
    "serialize",
    "base64",
    "hex",
    "url_rfc",
    "url_whatwg",
    "error",
    "obj",
];

/// The modules contributed by the `collections` aggregate package
/// (matches the former `CollectionsLibraryPlugin` sub-module set). The
/// `collections` aggregate [`Module`] itself is contributed separately —
/// see [`PackagePlugin::contribute`].
const COLLECTIONS_MODULES: &[&str] = &[
    "vec", "list", "queue", "stack", "deque", "hash_map", "hash_set",
];

/// The modules contributed by the `archive-tar` package (former
/// `plugin-lib-archive-tar`). The `archive` module is the caps-only
/// abstraction surface (former `plugin-lib-archive` `Archive` trait); the
/// `tar` module carries the callable `tar.pack` / `tar.unpack` ops.
const ARCHIVE_TAR_MODULES: &[&str] = &["archive", "tar"];

/// The package table — keyed by library-registry name (kit manifest
/// `modules:` entries).
pub static PACKAGES: &[PackageSpec] = &[
    PackageSpec {
        lib_name: "utils",
        plugin_id: "lib.utils",
        plugin_name: "lib-utils",
        hnl: include_str!("../../../handl-packages/utils/utils.hnl"),
        modules: &[], // resolved via UTILS_MODULES (see package_modules)
    },
    PackageSpec {
        lib_name: "cmp",
        plugin_id: "lib.cmp",
        plugin_name: "lib-cmp",
        hnl: include_str!("../../../handl-packages/cmp/cmp.hnl"),
        modules: &[],
    },


    PackageSpec {
        lib_name: "collections",
        plugin_id: "lib.collections",
        plugin_name: "lib-collections",
        hnl: include_str!("../../../handl-packages/collections/collections.hnl"),
        modules: &[], // resolved via COLLECTIONS_MODULES (see package_module_names)
    },
    PackageSpec {
        lib_name: "stats",
        plugin_id: "lib.stats",
        plugin_name: "lib-stats",
        hnl: include_str!("../../../handl-packages/stats/stats.hnl"),
        modules: &[],
    },
    // Single-module packages also usable standalone (idempotent with the
    // `utils` aggregate, which contributes the same modules).
    PackageSpec {
        lib_name: "base64",
        plugin_id: "lib.base64",
        plugin_name: "lib-base64",
        hnl: include_str!("../../../handl-packages/base64/base64.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "hex",
        plugin_id: "lib.hex",
        plugin_name: "lib-hex",
        hnl: include_str!("../../../handl-packages/hex/hex.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "url_rfc",
        plugin_id: "lib.url-rfc",
        plugin_name: "lib-url-rfc",
        hnl: include_str!("../../../handl-packages/url_rfc/url_rfc.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "url_whatwg",
        plugin_id: "lib.url-whatwg",
        plugin_name: "lib-url-whatwg",
        hnl: include_str!("../../../handl-packages/url_whatwg/url_whatwg.hnl"),
        modules: &[],
    },
    // ── hello / result / arrow_iter (former plugin-lib-{hello,result,
    // arrow-iter}) — pure-shell folds ─────────────────────────────────
    PackageSpec {
        lib_name: "hello",
        plugin_id: "lib.hello",
        plugin_name: "lib-hello",
        hnl: include_str!("../../../handl-packages/hello/greet.hnl"),
        modules: &[],
    },
    // NOTE: the `result` module's `tag` dep is not in the table — a kit
    // consuming `library("result")` must supply a resolver fallback that
    // maps `"tag"` (see the `result` [`ModuleSpec`] row above).
    PackageSpec {
        lib_name: "result",
        plugin_id: "lib.result",
        plugin_name: "lib-result",
        hnl: include_str!("../../../handl-packages/result/index.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "error",
        plugin_id: "lib.error",
        plugin_name: "lib-error",
        hnl: include_str!("../../../handl-packages/error/error.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "obj",
        plugin_id: "lib.obj",
        plugin_name: "lib-obj",
        hnl: include_str!("../../../handl-packages/obj/obj.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "arrow_iter",
        plugin_id: "lib.arrow-iter",
        plugin_name: "lib-arrow-iter",
        hnl: include_str!("../../../handl-packages/arrow_iter/arrow_iter.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "prim",
        plugin_id: "lib.prim",
        plugin_name: "lib-prim",
        hnl: include_str!("../../../handl-packages/prim/prim.hnl"),
        modules: &[],
    },
    // ── archive-tar (former plugin-lib-archive-tar) ─────────────────────
    PackageSpec {
        lib_name: "archive-tar",
        plugin_id: "lib.archive-tar",
        plugin_name: "lib-archive-tar",
        hnl: include_str!("../../../handl-packages/archive_tar/archive_tar.hnl"),
        modules: &[], // resolved via ARCHIVE_TAR_MODULES (see package_module_names)
    },
];

/// Look up a [`ModuleSpec`] by module name.
pub fn module_spec(name: &str) -> Option<&'static ModuleSpec> {
    MODULES.iter().find(|m| m.name == name)
}

/// The module names contributed by a package.
///
/// Most packages are single-module (the package name doubles as the module
/// name); `utils` is the aggregate with an explicit module list.
fn package_module_names(spec: &PackageSpec) -> Vec<&'static str> {
    if spec.lib_name == "utils" {
        return UTILS_MODULES.to_vec();
    }
    if spec.lib_name == "collections" {
        return COLLECTIONS_MODULES.to_vec();
    }
    if spec.lib_name == "archive-tar" {
        return ARCHIVE_TAR_MODULES.to_vec();
    }
    if !spec.modules.is_empty() {
        return spec.modules.iter().map(|m| m.name).collect();
    }
    vec![spec.lib_name]
}

// ---------------------------------------------------------------------------
// Module construction
// ---------------------------------------------------------------------------

/// A [`ModuleResolver`] that resolves names through the module table, then
/// through an optional fallback resolver (for non-table names like
/// `"char"`/`"unicode"` provided by the embedding host).
#[derive(Clone)]
pub struct PackageResolver {
    /// Fallback resolver consulted when the name is not in the table.
    pub fallback: Option<Arc<dyn ModuleResolver>>,
}

/// Normalize a path-like import spec to its module name.
///
/// Strips a leading `./`, strips a trailing `.hnl`, then takes the basename
/// after the last `/`: `./reverse.hnl` → `reverse`;
/// `collections/array/reverse.hnl` → `reverse`. Bare names such as
/// `"reverse"` are returned unchanged.
fn normalize_module_name(name: &str) -> &str {
    let name = name.strip_prefix("./").unwrap_or(name);
    let name = name.strip_suffix(".hnl").unwrap_or(name);
    name.rsplit('/').next().unwrap_or(name)
}

/// Resolve a (possibly path-like) module `name` through the module table,
/// building it with `resolver`.
///
/// Relative paths such as `./reverse.hnl` normalize to their module name;
/// bare names are matched directly. Returns `None` when neither form is in
/// the table (callers then consult their own fallback resolver).
fn resolve_in_table(
    name: &str,
    resolver: Arc<dyn ModuleResolver>,
) -> Option<Arc<dyn Module + Send + Sync>> {
    let normalized = normalize_module_name(name);
    let spec = module_spec(normalized).or_else(|| module_spec(name))?;
    Some(Arc::new(build_module(spec, resolver)))
}

impl ModuleResolver for PackageResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        resolve_in_table(name, Arc::new(self.clone()))
            .or_else(|| self.fallback.as_ref().and_then(|f| f.resolve(name)))
    }
}

/// The default table resolver (no fallback).
pub static DEFAULT_RESOLVER: PackageResolver = PackageResolver { fallback: None };

/// An `Arc` of the default table resolver.
fn default_resolver_arc() -> Arc<dyn ModuleResolver> {
    Arc::new(DEFAULT_RESOLVER.clone())
}

// ---------------------------------------------------------------------------
// TableImportResolver -- runtime evaluation of `import`/`from` statements
// ---------------------------------------------------------------------------

/// An [`ImportResolver`](handl_plugin_lib_handl::ImportResolver) that resolves
/// `.hnl` import statements through the **module table**.
///
/// The runtime evaluates `import`/`from` statements via `prim_import`, which
/// asks the resolver for the target module's bindings as a
/// `serde_json::Value::Object`. A handl closure cannot cross that boundary as
/// a callable, so each binding is the *string* `"<module>.<field>"` -- exactly
/// the callee form a qualified `mod.fn` already produces. The string is then
/// dispatched to the importer's module lookup, whose `<module>` entry resolves
/// through the dependency chain or the injected global table lookup. This keeps
/// `from "x" import * as m` (`m.f`) working *and* enables named imports
/// (`from "./x.hnl" import f` -> bare `f(...)`).
///
/// `path` is normalized to a module name ([`normalize_module_name`]) and the
/// module is built from the table -- cached, so repeated imports of the same
/// module never re-evaluate its source.
pub struct TableImportResolver {
    cache: Mutex<HashMap<String, Arc<serde_json::Value>>>,
    resolver: Arc<dyn ModuleResolver>,
}

impl TableImportResolver {
    /// Create an empty resolver with a fresh module-binding cache,
    /// using the default table resolver.
    pub fn new() -> Self {
        Self::with_resolver(default_resolver_arc())
    }

    /// Create a resolver that builds imported table modules with `resolver`,
    /// so the imports inherit the parent resolver (e.g. for `ffi`).
    pub fn with_resolver(resolver: Arc<dyn ModuleResolver>) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            resolver,
        }
    }
}

impl Default for TableImportResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_plugin_lib_handl::ImportResolver for TableImportResolver {
    fn resolve_import(
        &self,
        path: &str,
    ) -> Result<serde_json::Value, handl_language::LangError> {
        let name = normalize_module_name(path).to_string();
        if let Ok(cache) = self.cache.lock() {
            if let Some(value) = cache.get(&name) {
                return Ok((**value).clone());
            }
        }
        let spec = module_spec(&name).ok_or_else(|| {
            handl_language::LangError::engine(format!(
                "import: module `{name}` (path `{path}`) is not in the package table"
            ))
        })?;
        let module = build_module(spec, Arc::clone(&self.resolver));
        let mut obj = serde_json::Map::new();
        for (fname, _f) in module.functions() {
            obj.insert(
                fname.to_string(),
                serde_json::Value::String(format!("{}.{}", module.name(), fname)),
            );
        }
        let value = Arc::new(serde_json::Value::Object(obj));
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(name, Arc::clone(&value));
        }
        Ok((*value).clone())
    }
}

/// A [`ModuleLookup`] that resolves a string callee `"name.field"` through
/// the stdlib module table, building and caching the module on demand.
///
/// This is the *global* resolution path: a `.hnl` module's runtime consults
/// it when a string callee is not found in the module's own dependency
/// chain, so stdlib packages reach every table module without pre-listing
/// `deps`. Table modules resolve through [`module_ref`] (a process-wide
/// cache); names not in the table (e.g. `"tag"`, `"char"`) resolve through
/// the resolver this lookup was built with, leaked once per name so `lookup`
/// can return `&dyn Function` without holding a lock.
pub struct TableModuleLookup {
    resolver: Arc<dyn ModuleResolver>,
    fallback: Mutex<HashMap<String, &'static (dyn Module + Send + Sync)>>,
    in_progress: Mutex<HashSet<String>>,
}

impl TableModuleLookup {
    /// Create a lookup resolving table names via [`module_ref`] and non-table
    /// names via `resolver`.
    pub fn new(resolver: Arc<dyn ModuleResolver>) -> Self {
        Self {
            resolver,
            fallback: Mutex::new(HashMap::new()),
            in_progress: Mutex::new(HashSet::new()),
        }
    }

    /// Resolve a non-table module name, building it once and caching the
    /// leaked `'static` reference. Cycle-protected via `in_progress`.
    fn resolve_fallback(&self, name: &str) -> Option<&'static (dyn Module + Send + Sync)> {
        let normalized = normalize_module_name(name);
        if let Ok(cache) = self.fallback.lock() {
            if let Some(module) = cache.get(normalized) {
                return Some(*module);
            }
        }
        {
            let mut in_progress = self.in_progress.lock().ok()?;
            if !in_progress.insert(normalized.to_string()) {
                // Already building this module further up the stack: a cycle.
                return None;
            }
        }
        let built = self.resolver.resolve(normalized);
        if let Ok(mut in_progress) = self.in_progress.lock() {
            in_progress.remove(normalized);
        }
        let module = built?;
        let boxed: &'static Arc<dyn Module + Send + Sync> = Box::leak(Box::new(module));
        let leaked: &'static (dyn Module + Send + Sync) = &**boxed;
        if let Ok(mut cache) = self.fallback.lock() {
            cache.entry(normalized.to_string()).or_insert(leaked);
            return cache.get(normalized).copied();
        }
        Some(leaked)
    }
}

impl ModuleLookup for TableModuleLookup {
    fn lookup(&self, qualified_name: &str) -> Option<&dyn Function> {
        let (module_name, func_name) = qualified_name.split_once('.')?;
        // Prefer the resolver this lookup was built with so table modules
        // that import non-table names (e.g. `clock_gettime` -> `ffi`) are
        // built with the parent resolver.  Fall back to the process-wide
        // `module_ref` cache for pure-table names.
        if let Some(module) = self.resolve_fallback(module_name) {
            return module.function(func_name);
        }
        module_ref(module_name)?.function(func_name)
    }
}

/// Build a [`HandlModule`] from a [`ModuleSpec`], wiring op dispatch and an
/// injected global [`TableModuleLookup`]. Resolution is global, so the module
/// reaches every other table module (and the resolver's non-table modules)
/// through the lookup.
pub fn build_module(spec: &ModuleSpec, resolver: Arc<dyn ModuleResolver>) -> HandlModule {
    let lookup = Arc::new(TableModuleLookup::new(Arc::clone(&resolver)));
    let mut module = HandlModule::new(spec.name, spec.hnl, spec.funcs)
        .with_import_resolver(Arc::new(TableImportResolver::with_resolver(Arc::clone(
            &resolver,
        ))))
        .with_global_lookup(lookup);
    if let Some(registry) = spec.registry {
        module = module.with_ops(spec.ops, Arc::new(registry()));
    }
    module
}

/// Build the table module named `name` (e.g. `"bool"`, `"math"`,
/// `"stats"`), resolving its deps globally through the table itself.
pub fn module(name: &str) -> Option<HandlModule> {
    module_spec(name).map(|spec| build_module(spec, default_resolver_arc()))
}

/// Build the table module named `name`, wiring `resolver` as the global
/// lookup's non-table fallback (for hosts that provide non-table modules).
pub fn module_with(name: &str, resolver: Arc<dyn ModuleResolver>) -> Option<HandlModule> {
    module_spec(name).map(|spec| build_module(spec, resolver))
}

/// Shared, lazily-built table modules — the replacement for the deleted
/// per-crate `OnceLock<HandlModule>` caches.
fn module_cache() -> &'static Mutex<HashMap<String, &'static HandlModule>> {
    static CACHE: OnceLock<Mutex<HashMap<String, &'static HandlModule>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get a shared `&'static` [`HandlModule`] for the table module `name`,
/// building it on first access.
pub fn module_ref(name: &str) -> Option<&'static HandlModule> {
    module_spec(name)?;
    let mut cache = module_cache().lock().ok()?;
    Some(
        *cache.entry(name.to_string()).or_insert_with(|| {
            Box::leak(Box::new(module(name).expect("module_spec checked above")))
        }),
    )
}

// ---------------------------------------------------------------------------
// PackagePlugin — the generic per-package Plugin
// ---------------------------------------------------------------------------

/// A [`Plugin`] built from a [`PackageSpec`] — contributes each of the
/// package's modules into the [`ModuleRuntime`].
pub struct PackagePlugin {
    spec: &'static PackageSpec,
    caps: Vec<CapabilityId>,
    /// Optional resolver for module deps (e.g. kit-provided modules).
    resolver_fallback: Option<Arc<dyn ModuleResolver>>,
}

impl PackagePlugin {
    /// Create a plugin for `spec`, parsing `cap` declarations from the
    /// package `.hnl` interface.
    pub fn new(spec: &'static PackageSpec) -> Self {
        Self {
            caps: parse_cap_declarations(spec.hnl)
                .into_iter()
                .map(CapabilityId::from)
                .collect(),
            spec,
            resolver_fallback: None,
        }
    }

    /// Provide a fallback [`ModuleResolver`] for dep names not in the table.
    pub fn with_resolver_fallback(mut self, resolver: Arc<dyn ModuleResolver>) -> Self {
        self.resolver_fallback = Some(resolver);
        self
    }
}

impl Plugin for PackagePlugin {
    fn id(&self) -> &str {
        self.spec.plugin_id
    }

    fn name(&self) -> &str {
        self.spec.plugin_name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.caps
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        #[derive(Clone)]
        struct FallbackResolver {
            inner: Option<Arc<dyn ModuleResolver>>,
        }
        impl ModuleResolver for FallbackResolver {
            fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
                resolve_in_table(name, Arc::new(self.clone()))
                    .or_else(|| self.inner.as_ref().and_then(|f| f.resolve(name)))
            }
        }
        let fallback: Arc<dyn ModuleResolver> = Arc::new(FallbackResolver {
            inner: self.resolver_fallback.clone(),
        });
        for name in package_module_names(self.spec) {
            let spec = module_spec(name).unwrap_or_else(|| {
                panic!(
                    "package '{}' lists unknown module '{name}'",
                    self.spec.lib_name
                )
            });
            ctx.contribute::<ModuleRuntime>(ModuleRule::new(build_module(
                spec,
                Arc::clone(&fallback),
            )))?;
        }
        if self.spec.lib_name == "collections" {
            // Also contribute the `collections` aggregate module so
            // `collections.vec.new`-style lookups keep working (the former
            // `CollectionsLibraryPlugin` registered only this aggregate).
            ctx.contribute::<ModuleRuntime>(ModuleRule::new(
                ops::collections::CollectionsModule::new(Arc::clone(&fallback)),
            ))?;
        }
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// Build a `Box<dyn Plugin>` for the package named `lib_name`
/// (e.g. `"utils"`, `"cmp"`, `"stats"`), or `None` if unknown.
pub fn library(lib_name: &str) -> Option<Box<dyn Plugin + Send + Sync>> {
    PACKAGES
        .iter()
        .find(|p| p.lib_name == lib_name)
        .map(|spec| Box::new(PackagePlugin::new(spec)) as Box<dyn Plugin + Send + Sync>)
}

#[cfg(test)]
mod tests {
    use super::normalize_module_name;

    #[test]
    fn normalizes_relative_import_paths() {
        // Relative path with extension and leading `./`.
        assert_eq!(normalize_module_name("./reverse.hnl"), "reverse");
        // Bare name is unchanged.
        assert_eq!(normalize_module_name("reverse"), "reverse");
        // Deep path keeps only the basename (minus extension).
        assert_eq!(normalize_module_name("a/b/c.hnl"), "c");
        // Nested `collections/array` path.
        assert_eq!(normalize_module_name("collections/array/reverse.hnl"), "reverse");
        // Private module name.
        assert_eq!(normalize_module_name("./_reflect.hnl"), "_reflect");
    }
}
