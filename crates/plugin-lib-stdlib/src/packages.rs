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

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    Module, PlatformError, Plugin, PluginContext,
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
    /// Names of dependency modules to register in the bridge runtime
    /// (resolved through the ambient [`ModuleResolver`]).
    pub deps: &'static [&'static str],
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

/// All table-built modules, keyed by module name.
pub static MODULES: &[ModuleSpec] = &[
    // ── utils (former plugin-lib-utils) ────────────────────────────────
    ModuleSpec {
        name: "bool",
        hnl: include_str!("../../../handl-packages/utils/bool.hnl"),
        funcs: &["and", "or", "not", "xor", "nand", "nor", "implies", "eq"],
        ops: &[],
        registry: None,
        deps: &[],
    },
    ModuleSpec {
        name: "math",
        hnl: include_str!("../../../handl-packages/utils/math.hnl"),
        funcs: &[],
        ops: ops::math::MATH_OPS,
        registry: Some(ops::math::math_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "str",
        hnl: include_str!("../../../handl-packages/utils/str.hnl"),
        funcs: &[],
        ops: ops::strs::STR_OPS,
        registry: Some(ops::strs::str_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "combinators",
        hnl: include_str!("../../../handl-packages/utils/fn.hnl"),
        funcs: &[],
        ops: ops::combinators::COMBINATORS_OPS,
        registry: Some(ops::combinators::combinators_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "transform",
        hnl: include_str!("../../../handl-packages/transform/transform.hnl"),
        funcs: &[],
        ops: ops::combinators::TRANSFORM_OPS,
        registry: Some(ops::combinators::transform_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "typeof",
        hnl: include_str!("../../../handl-packages/utils/type.hnl"),
        funcs: &[],
        ops: ops::type_ops::TYPE_OPS,
        registry: Some(ops::type_ops::type_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "option",
        hnl: include_str!("../../../handl-packages/utils/option.hnl"),
        funcs: &[],
        ops: ops::option::OPTION_OPS,
        registry: Some(ops::option::option_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "mood",
        hnl: include_str!("../../../handl-packages/utils/mood.hnl"),
        funcs: &[],
        ops: ops::mood::MOOD_OPS,
        registry: Some(ops::mood::mood_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "encoding",
        hnl: include_str!("../../../handl-packages/utils/encoding.hnl"),
        funcs: &[],
        ops: ops::encoding::ENCODING_OPS,
        registry: Some(ops::encoding::encoding_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "parse",
        hnl: include_str!("../../../handl-packages/utils/parse.hnl"),
        funcs: &[],
        ops: ops::parse::PARSE_OPS,
        registry: Some(ops::parse::parse_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "serialize",
        hnl: include_str!("../../../handl-packages/utils/serialize.hnl"),
        funcs: &[],
        ops: ops::serialize::SERIALIZE_OPS,
        registry: Some(ops::serialize::serialize_op_registry),
        deps: &[],
    },
    // ── codecs / formats (former plugin-lib-{base64,hex,json,toml,yaml}) ──
    ModuleSpec {
        name: "base64",
        hnl: include_str!("../../../handl-packages/base64/base64.hnl"),
        funcs: &[],
        ops: ops::base64::BASE64_OPS,
        registry: Some(ops::base64::base64_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "hex",
        hnl: include_str!("../../../handl-packages/hex/hex.hnl"),
        funcs: &[],
        ops: ops::hex::HEX_OPS,
        registry: Some(ops::hex::hex_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "json",
        hnl: include_str!("../../../handl-packages/json/json.hnl"),
        funcs: &[],
        ops: ops::json::JSON_OPS,
        registry: Some(ops::json::json_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "toml",
        hnl: include_str!("../../../handl-packages/toml/toml.hnl"),
        funcs: &[],
        ops: ops::toml::TOML_OPS,
        registry: Some(ops::toml::toml_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "yaml",
        hnl: include_str!("../../../handl-packages/yaml/yaml.hnl"),
        funcs: &[],
        ops: ops::yaml::YAML_OPS,
        registry: Some(ops::yaml::yaml_op_registry),
        deps: &[],
    },
    // ── url parsers (former plugin-lib-url-{rfc,whatwg}) ───────────────
    ModuleSpec {
        name: "url_rfc",
        hnl: include_str!("../../../handl-packages/url_rfc/url_rfc.hnl"),
        funcs: &[],
        ops: ops::url_rfc::URL_RFC_OPS,
        registry: Some(ops::url_rfc::url_rfc_op_registry),
        deps: &[],
    },
    ModuleSpec {
        name: "url_whatwg",
        hnl: include_str!("../../../handl-packages/url_whatwg/url_whatwg.hnl"),
        funcs: &[],
        ops: ops::url_whatwg::URL_WHATWG_OPS,
        registry: Some(ops::url_whatwg::url_whatwg_op_registry),
        deps: &[],
    },
    // ── cmp (former plugin-lib-cmp) ────────────────────────────────────
    ModuleSpec {
        name: "cmp",
        hnl: include_str!("../../../handl-packages/cmp/cmp.hnl"),
        funcs: &[],
        ops: ops::cmp::CMP_OPS,
        registry: Some(ops::cmp::cmp_op_registry),
        deps: &[],
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
        deps: &["error"],
    },
    ModuleSpec {
        name: "list",
        hnl: include_str!("../../../handl-packages/collections/list.hnl"),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::LIST_OPS,
        registry: Some(ops::collections::collections_op_registry),
        deps: &["error"],
    },
    ModuleSpec {
        name: "queue",
        hnl: include_str!("../../../handl-packages/collections/queue.hnl"),
        funcs: &["len", "is_empty", "enqueue", "dequeue", "peek"],
        ops: ops::collections::QUEUE_OPS,
        registry: Some(ops::collections::collections_op_registry),
        deps: &["error"],
    },
    ModuleSpec {
        name: "stack",
        hnl: include_str!("../../../handl-packages/collections/stack.hnl"),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::STACK_OPS,
        registry: Some(ops::collections::collections_op_registry),
        deps: &["error"],
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
        deps: &["error"],
    },
    // `hash_map`/`hash_set` stay mostly native (see the module .hnl docs);
    // the field-enumeration surface (`keys`/`values`/`entries`) is now
    // pure handl over the `obj` introspection primitives.
    ModuleSpec {
        name: "hash_map",
        hnl: include_str!("../../../handl-packages/collections/hash_map.hnl"),
        funcs: &["keys", "values", "entries"],
        ops: ops::collections::HASH_MAP_OPS,
        registry: Some(ops::collections::collections_op_registry),
        deps: &["obj"],
    },
    ModuleSpec {
        name: "hash_set",
        hnl: include_str!("../../../handl-packages/collections/hash_set.hnl"),
        funcs: &[],
        ops: ops::collections::HASH_SET_OPS,
        registry: Some(ops::collections::collections_op_registry),
        deps: &[],
    },
    // `array` — the arrow-free surface is pure handl; the arrow-taking
    // members (`take_while`/`drop_while`/`scan`/`foldr`/`find_*`) are ops.
    ModuleSpec {
        name: "array",
        hnl: include_str!("../../../handl-packages/collections/array.hnl"),
        funcs: &[
            "len",
            "is_empty",
            "reverse",
            "concat",
            "contains",
            "index_of",
            "slice",
            "first",
            "last",
            "at",
            "pop",
            "shift",
            "fill",
            "flatten",
            "deep_flatten",
            "take",
            "drop",
            "enumerate",
            "unzip",
            "intersperse",
            "intercalate",
            "dedupe",
            "prepend",
            "windows",
            "transpose",
            "insert_at",
            "remove_at",
            "zip",
            "chunk",
            "compact",
            "pluck",
        ],
        ops: ops::collections::ARRAY_OPS,
        registry: Some(ops::collections::collections_op_registry),
        deps: &["typeof", "hash_map"],
    },
    // `array` per-arrow modules (`collections/array/<name>.hnl`). Each
    // arrow lives in its own file (`export <name> = ...`, its own doc
    // comment); `array.hnl` (the aggregator) imports them all via
    // `from "collections/array/<name>" import * as <name>`.
    ModuleSpec { name: "len", hnl: include_str!("../../../handl-packages/collections/array/len.hnl"), funcs: &["len"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "is_empty", hnl: include_str!("../../../handl-packages/collections/array/is_empty.hnl"), funcs: &["is_empty"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "reverse", hnl: include_str!("../../../handl-packages/collections/array/reverse.hnl"), funcs: &["reverse"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "concat", hnl: include_str!("../../../handl-packages/collections/array/concat.hnl"), funcs: &["concat"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "contains", hnl: include_str!("../../../handl-packages/collections/array/contains.hnl"), funcs: &["contains"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "index_of", hnl: include_str!("../../../handl-packages/collections/array/index_of.hnl"), funcs: &["index_of"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "slice", hnl: include_str!("../../../handl-packages/collections/array/slice.hnl"), funcs: &["slice"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "first", hnl: include_str!("../../../handl-packages/collections/array/first.hnl"), funcs: &["first"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "last", hnl: include_str!("../../../handl-packages/collections/array/last.hnl"), funcs: &["last"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "at", hnl: include_str!("../../../handl-packages/collections/array/at.hnl"), funcs: &["at"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "pop", hnl: include_str!("../../../handl-packages/collections/array/pop.hnl"), funcs: &["pop"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "shift", hnl: include_str!("../../../handl-packages/collections/array/shift.hnl"), funcs: &["shift"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "fill", hnl: include_str!("../../../handl-packages/collections/array/fill.hnl"), funcs: &["fill"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "flatten", hnl: include_str!("../../../handl-packages/collections/array/flatten.hnl"), funcs: &["flatten"], ops: &[], registry: None, deps: &["typeof"] },
    ModuleSpec { name: "deep_flatten", hnl: include_str!("../../../handl-packages/collections/array/deep_flatten.hnl"), funcs: &["deep_flatten"], ops: &[], registry: None, deps: &["typeof"] },
    ModuleSpec { name: "take", hnl: include_str!("../../../handl-packages/collections/array/take.hnl"), funcs: &["take"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "drop", hnl: include_str!("../../../handl-packages/collections/array/drop.hnl"), funcs: &["drop"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "enumerate", hnl: include_str!("../../../handl-packages/collections/array/enumerate.hnl"), funcs: &["enumerate"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "unzip", hnl: include_str!("../../../handl-packages/collections/array/unzip.hnl"), funcs: &["unzip"], ops: &[], registry: None, deps: &["typeof"] },
    ModuleSpec { name: "intersperse", hnl: include_str!("../../../handl-packages/collections/array/intersperse.hnl"), funcs: &["intersperse"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "intercalate", hnl: include_str!("../../../handl-packages/collections/array/intercalate.hnl"), funcs: &["intercalate"], ops: &[], registry: None, deps: &["typeof"] },
    ModuleSpec { name: "dedupe", hnl: include_str!("../../../handl-packages/collections/array/dedupe.hnl"), funcs: &["dedupe"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "prepend", hnl: include_str!("../../../handl-packages/collections/array/prepend.hnl"), funcs: &["prepend"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "windows", hnl: include_str!("../../../handl-packages/collections/array/windows.hnl"), funcs: &["windows"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "transpose", hnl: include_str!("../../../handl-packages/collections/array/transpose.hnl"), funcs: &["transpose"], ops: &[], registry: None, deps: &["typeof"] },
    ModuleSpec { name: "insert_at", hnl: include_str!("../../../handl-packages/collections/array/insert_at.hnl"), funcs: &["insert_at"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "remove_at", hnl: include_str!("../../../handl-packages/collections/array/remove_at.hnl"), funcs: &["remove_at"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "zip", hnl: include_str!("../../../handl-packages/collections/array/zip.hnl"), funcs: &["zip"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "chunk", hnl: include_str!("../../../handl-packages/collections/array/chunk.hnl"), funcs: &["chunk"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "compact", hnl: include_str!("../../../handl-packages/collections/array/compact.hnl"), funcs: &["compact"], ops: &[], registry: None, deps: &[] },
    ModuleSpec { name: "pluck", hnl: include_str!("../../../handl-packages/collections/array/pluck.hnl"), funcs: &["pluck"], ops: &[], registry: None, deps: &["typeof", "hash_map"] },
    // ── stats (former plugin-lib-stats) — pure handl ───────────────────
    ModuleSpec {
        name: "stats",
        hnl: include_str!("../../../handl-packages/stats/stats.hnl"),
        funcs: &["sum", "avg", "product", "min", "max", "median", "stddev"],
        ops: &[],
        registry: None,
        deps: &["math", "typeof"],
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
        deps: &[],
    },
    // ── result (former plugin-lib-result) — pure handl over `tag.*` ───
    // `result.ok`/`result.err` are `tag` constructor arrows: the
    // `tag ok = Value` / `tag err = Error` declarations expand to
    // `tag.tag("ok", {value: _0})` / `tag.tag("err", {value: _0})` calls,
    // so the `tag` module must resolve through the ambient
    // [`ModuleResolver`]. The default table resolver does not know
    // `"tag"` (it stays in `plugin-lib-tag`); callers building the
    // `result` module must pass a resolver that maps it — e.g.
    // `module_with("result", &resolver)` with kit-canonical's
    // `StdlibResolver`, or `PackagePlugin::with_resolver_fallback`.
    ModuleSpec {
        name: "result",
        hnl: include_str!("../../../handl-packages/result/result.hnl"),
        funcs: &["ok", "err"],
        ops: &[],
        registry: None,
        deps: &["tag"],
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
        deps: &[],
    },
    // ── error (ADR-0092 gap (a)) — the error-raising primitive ─────────
    // `error.raise` is an irreducible atom: aborting evaluation cannot be
    // expressed over the eval vocabulary, so the module is ops-only. Any
    // `.hnl` module that needs to fail loudly declares `deps: &["error"]`
    // (or registers the module in its ambient chain) and calls
    // `error.raise(msg)`.
    ModuleSpec {
        name: "error",
        hnl: include_str!("../../../handl-packages/error/error.hnl"),
        funcs: &[],
        ops: ops::error::ERROR_OPS,
        registry: Some(ops::error::error_op_registry),
        deps: &[],
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
        deps: &[],
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
        deps: &["math", "cmp"],
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
        deps: &[],
    },
    ModuleSpec {
        name: "tar",
        hnl: include_str!("../../../handl-packages/archive_tar/archive_tar.hnl"),
        funcs: &[],
        ops: ops::tar::TAR_OPS,
        registry: Some(ops::tar::tar_op_registry),
        deps: &[],
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
    "json",
    "toml",
    "yaml",
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
    "vec", "list", "queue", "stack", "deque", "hash_map", "hash_set", "array",
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
        lib_name: "json",
        plugin_id: "lib.json",
        plugin_name: "lib-json",
        hnl: include_str!("../../../handl-packages/json/json.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "toml",
        plugin_id: "lib.toml",
        plugin_name: "lib-toml",
        hnl: include_str!("../../../handl-packages/toml/toml.hnl"),
        modules: &[],
    },
    PackageSpec {
        lib_name: "yaml",
        plugin_id: "lib.yaml",
        plugin_name: "lib-yaml",
        hnl: include_str!("../../../handl-packages/yaml/yaml.hnl"),
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
        hnl: include_str!("../../../handl-packages/result/result.hnl"),
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
pub struct PackageResolver<'a> {
    /// Fallback resolver consulted when the name is not in the table.
    pub fallback: Option<&'a dyn ModuleResolver>,
}

impl ModuleResolver for PackageResolver<'_> {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        if let Some(spec) = module_spec(name) {
            return Some(Arc::new(build_module(spec, self)));
        }
        self.fallback.and_then(|f| f.resolve(name))
    }
}

/// The default table resolver (no fallback).
pub static DEFAULT_RESOLVER: PackageResolver<'static> = PackageResolver { fallback: None };

/// Build a [`HandlModule`] from a [`ModuleSpec`], wiring op dispatch and
/// dependency modules through `resolver`.
pub fn build_module(spec: &ModuleSpec, resolver: &dyn ModuleResolver) -> HandlModule {
    let mut module = HandlModule::new(spec.name, spec.hnl, spec.funcs).with_resolver(resolver);
    for dep in spec.deps {
        if let Some(dep_module) = resolver.resolve(dep) {
            module = module.with_dependency_arc(dep_module);
        }
    }
    if let Some(registry) = spec.registry {
        module = module.with_ops(spec.ops, Arc::new(registry()));
    }
    module
}

/// Build the table module named `name` (e.g. `"bool"`, `"math"`,
/// `"stats"`), resolving its declared deps through the table itself.
pub fn module(name: &str) -> Option<HandlModule> {
    module_spec(name).map(|spec| build_module(spec, &DEFAULT_RESOLVER))
}

/// Build the table module named `name`, resolving deps through
/// `resolver` (for hosts that provide non-table modules).
pub fn module_with(name: &str, resolver: &dyn ModuleResolver) -> Option<HandlModule> {
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
        struct FallbackResolver<'a> {
            inner: Option<&'a dyn ModuleResolver>,
        }
        impl ModuleResolver for FallbackResolver<'_> {
            fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
                if let Some(spec) = module_spec(name) {
                    return Some(Arc::new(build_module(spec, self)));
                }
                self.inner.and_then(|f| f.resolve(name))
            }
        }
        let fallback = FallbackResolver {
            inner: self.resolver_fallback.as_deref(),
        };
        for name in package_module_names(self.spec) {
            let spec = module_spec(name).unwrap_or_else(|| {
                panic!(
                    "package '{}' lists unknown module '{name}'",
                    self.spec.lib_name
                )
            });
            ctx.contribute::<ModuleRuntime>(ModuleRule::new(build_module(spec, &fallback)))?;
        }
        if self.spec.lib_name == "collections" {
            // Also contribute the `collections` aggregate module so
            // `collections.vec.new`-style lookups keep working (the former
            // `CollectionsLibraryPlugin` registered only this aggregate).
            ctx.contribute::<ModuleRuntime>(ModuleRule::new(
                ops::collections::CollectionsModule::new(&fallback),
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
