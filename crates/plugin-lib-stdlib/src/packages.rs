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
use handl_plugin_lib_handl::{
    HandlModule, ModuleEnv, ModuleResolver, OpRegistry, parse_cap_declarations,
};
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
    /// Record-field value names exposed by the source — non-arrow exports
    /// bound as evaluated values by `from "<mod>" import <name>` (e.g.
    /// `bool`'s `true`/`false`). The names are explicit because telling
    /// value fields from arrow fields requires evaluating the record.
    pub values: &'static [&'static str],
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
    hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-time-c/src/clock_gettime.hnl")),
    funcs: &["read_ns"],
    ops: &[],
    values: &[],
    registry: None,
};

const ENTROPY_MODULE: ModuleSpec = ModuleSpec {
    name: "entropy",
    hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-rand-c/src/entropy.hnl")),
    funcs: &["next_u32"],
    ops: &[],
    values: &[],
    registry: None,
};

// ---------------------------------------------------------------------------
// Owned `.hnl` pins re-exported for external consumers (ADR-0128 §".hnl
// ownership": one `include_str!` per package, in the crate that owns it;
// every other consumer takes a `pub const`). This crate owns the `hello` and
// `transform` package dirs (they are rows in the `PACKAGES`/`MODULES` tables
// below and move with `stdlib/` in Phase 3), so the pins live here;
// `handl-kit-canonical` (in the `kit-canonical/` repo) and
// `plugin-lib-handl`'s tests consume these consts instead of their own
// `include_str!`s.
// ---------------------------------------------------------------------------

/// Raw source of `handl-packages/hello/greet.hnl` — a bare arrow
/// (`name ~> "Hello, " + name + "!"`), consumed by `handl-kit-canonical`'s
/// `greet` bridge.
pub use handl_lib_hello::GREET_HNL;

/// Source of the transform sub-ops (`lib-transform`).
pub use handl_lib_transform::{
    FILTER_HNL as TRANSFORM_FILTER_HNL, MAP_HNL as TRANSFORM_MAP_HNL,
    REDUCE_HNL as TRANSFORM_REDUCE_HNL,
};

/// All table-built modules, keyed by module name.
pub static MODULES: &[ModuleSpec] = &[
    CLOCK_GETTIME_MODULE,
    ENTROPY_MODULE,
    // ── utils (former plugin-lib-utils) ────────────────────────────────
    ModuleSpec {
        name: "bool",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-utils/src/bool.hnl")),
        funcs: &["and", "or", "not", "xor", "nand", "nor", "implies", "eq"],
        ops: &[],
        values: &["true", "false"],
        registry: None,
    },
    ModuleSpec {
        name: "math",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-utils/src/math.hnl")),
        funcs: &[],
        ops: ops::math::MATH_OPS,
        values: &[],
        registry: Some(ops::math::math_op_registry),
    },
    ModuleSpec {
        name: "str",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-utils/src/str.hnl")),
        funcs: &[],
        ops: ops::strs::STR_OPS,
        values: &[],
        registry: Some(ops::strs::str_op_registry),
    },
    ModuleSpec {
        name: "combinators",
        hnl: handl_lib_combinators::COMBINATORS_HNL,
        funcs: &[],
        ops: handl_lib_combinators::COMBINATORS_OPS,
        values: &[],
        registry: Some(handl_lib_combinators::combinators_op_registry),
    },
    ModuleSpec {
        name: "transform",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-transform/src/index.hnl")),
        funcs: &[],
        ops: handl_lib_combinators::TRANSFORM_OPS,
        values: &[],
        registry: Some(handl_lib_combinators::transform_op_registry),
    },
    ModuleSpec {
        name: "typeof",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-type/src/index.hnl")),
        funcs: &[],
        ops: handl_lib_type::TYPE_OPS,
        values: &[],
        registry: Some(handl_lib_type::type_op_registry),
    },
    ModuleSpec {
        name: "option",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-option/src/index.hnl")),
        // `Some`/`None` are exported under their ctor names too (phase-3
        // WS-F `__ctor__` mint): `Some` aliases the `some` arrow and
        // `None` binds the nullary ctor object `match` discriminates on.
        funcs: &["Option", "some", "none", "Some", "None"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "mood",
        hnl: handl_lib_mood::MOOD_HNL,
        funcs: &[],
        ops: handl_lib_mood::MOOD_OPS,
        values: &[],
        registry: Some(handl_lib_mood::mood_op_registry),
    },
    ModuleSpec {
        name: "encoding",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-encoding/src/index.hnl")),
        funcs: &["encode", "decode"],
        ops: handl_lib_encoding::ENCODING_OPS,
        values: &[],
        registry: None,
    },
    // ── bytes ──────────────────────────────────────────────────────────
    // Byte-buffer primitives for the `Bytes` scalar type — the only
    // .hnl-side constructor (previously only FFI `out_buf` produced them).
    // Ops-only module: every function is an irreducible atom.
    ModuleSpec {
        name: "bytes",
        hnl: handl_lib_bytes::BYTES_HNL,
        funcs: &[],
        ops: handl_lib_bytes::BYTES_OPS,
        values: &[],
        registry: Some(handl_lib_bytes::bytes_op_registry),
    },
    // ── datetime (pure handl, formerly plugin-lib-datetime Rust crate) ──────
    // Moved to handl-packages/datetime/ as pure handl over math/str/char/typeof.
    // No `ops` table needed — the handl module is a self-contained record.
    // Registration: stdlib::module("datetime") builds HandlModule from the
    // include_str! below + any dep modules the handl source imports.
    ModuleSpec {
        name: "datetime",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-datetime/src/index.hnl")),
        funcs: &[],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "parse",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-parse/src/index.hnl")),
        funcs: &[],
        ops: handl_lib_parse::PARSE_OPS,
        values: &[],
        registry: None,
    },
    // ── crypto — pure-handl SHA-1/SHA-256/MD5 over math/bytes/typeof ─────
    ModuleSpec {
        name: "crypto",
        hnl: handl_plugin_lib_crypto::CRYPTO_HNL,
        funcs: handl_plugin_lib_crypto::CRYPTO_FUNCS,
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "otlp",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-otlp/src/index.hnl")),
        funcs: &["configure","start_span","end_span","set_attribute","add_event", "record_log","record_metric","flush","shutdown",],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "otlp_c",
        hnl: handl_plugin_lib_otlp_c::OTLP_C_HNL,
        funcs: &[],
        ops: handl_plugin_lib_otlp_c::OTLP_OPS,
        values: &[],
        registry: Some(handl_plugin_lib_otlp_std::otlp_std_op_registry),
    },
    ModuleSpec {
        name: "kv_map",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-kv-map/src/index.hnl")),
        funcs: &["new", "set", "get", "has", "remove", "clear", "size", "keys"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "json",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-json/src/index.hnl")),
        funcs: &["stringify", "parse", "json"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ip",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-ip/src/index.hnl")),
        funcs: &["parse","format","is_loopback","is_multicast","is_private","is_link_local","is_unspecified", "cidr.parse","cidr.format","cidr.contains","cidr.network","cidr.broadcast", "socket.parse","socket.format",],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "serialize",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-serialize/src/index.hnl")),
        funcs: &["serialize","deserialize","toml","yaml"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── codecs / formats (former plugin-lib-{base64,hex,json,toml,yaml}) ──
    ModuleSpec {
        name: "base64",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-base64/src/index.hnl")),
        funcs: &["encode","decode","codec"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "hex",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-hex/src/index.hnl")),
        funcs: &["encode","decode","codec"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── serialize backends (lib-serialize-{toml,yaml}-c) ──────────────
    // Native `toml_c.*`/`yaml_c.*` ops — the Rust `Function` impls extracted
    // from `ops::toml`/`ops::yaml` into their own `-c` crates. The
    // `serialize` contract's `toml()`/`yaml()` producers build serializer
    // records whose arrow fields call these ops.
    ModuleSpec {
        name: "toml_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-serialize-toml-c/src/index.hnl")),
        funcs: &[],
        ops: handl_plugin_lib_serialize_toml_c::TOML_C_OPS,
        values: &[],
        registry: Some(handl_plugin_lib_serialize_toml_c::toml_c_op_registry),
    },
    ModuleSpec {
        name: "yaml_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-serialize-yaml-c/src/index.hnl")),
        funcs: &[],
        ops: handl_plugin_lib_serialize_yaml_c::YAML_C_OPS,
        values: &[],
        registry: Some(handl_plugin_lib_serialize_yaml_c::yaml_c_op_registry),
    },
    // ── url parsers (former plugin-lib-url-{rfc,whatwg}) ───────────────
    ModuleSpec {
        name: "url_rfc",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-url-rfc/src/index.hnl")),
        funcs: &["parse","url_rfc"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "url_whatwg",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-url-whatwg/src/index.hnl")),
        funcs: &["parse","url_whatwg"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── cmp (former plugin-lib-cmp) ────────────────────────────────────
    ModuleSpec {
        name: "cmp",
        hnl: handl_lib_cmp::CMP_HNL,
        funcs: &[],
        ops: handl_lib_cmp::CMP_OPS,
        values: &[],
        registry: Some(handl_lib_cmp::cmp_op_registry),
    },
    // ── collections (former plugin-lib-collections) ────────────────────
    // Sequential containers: the whole non-nullary surface is pure handl
    // over plain arrays; `ops` holds only `new`/`from`/`to_array`
    // (`new` is nullary, `from` is a reserved word). `pop`/`peek`/
    // `dequeue`/`pop_*`/`peek_*` raise `collection is empty` via
    // `error.raise` — the `error` dep resolves through the table.
    ModuleSpec {
        name: "vec",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/vec.hnl")),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::VEC_OPS,
        values: &[],
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "list",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/list/index.hnl")),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::LIST_OPS,
        values: &[],
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
    ModuleSpec {
        name: "list_len",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/list/len.hnl")),
        funcs: &["len"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "list_is_empty",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/list/is_empty.hnl")),
        funcs: &["is_empty"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "list_push",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/list/push.hnl")),
        funcs: &["push"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "list_peek",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/list/peek.hnl")),
        funcs: &["peek"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "list_pop",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/list/pop.hnl")),
        funcs: &["pop"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "queue",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/queue.hnl")),
        funcs: &["len", "is_empty", "enqueue", "dequeue", "peek"],
        ops: ops::collections::QUEUE_OPS,
        values: &[],
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "stack",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/stack.hnl")),
        funcs: &["len", "is_empty", "push", "pop", "peek"],
        ops: ops::collections::STACK_OPS,
        values: &[],
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "deque",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/deque.hnl")),
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
        values: &[],
        registry: Some(ops::collections::collections_op_registry),
    },
    // `hash_map`/`hash_set` stay mostly native (see the module .hnl docs);
    // the field-enumeration surface (`keys`/`values`/`entries`) is now
    // pure handl over the `obj` introspection primitives.
    ModuleSpec {
        name: "hash_map",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/hash_map.hnl")),
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
        values: &[],
        registry: Some(ops::collections::collections_op_registry),
    },
    ModuleSpec {
        name: "hash_set",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/hash_set.hnl")),
        funcs: &[],
        ops: ops::collections::HASH_SET_OPS,
        values: &[],
        registry: Some(ops::collections::collections_op_registry),
    },
    // The `array` package is loaded from `handl-packages/collections/array/index.hnl`
    // at runtime via `crate::loader::load_hnl_file` instead of being embedded here.
    // `hash_map` still depends on the leaf `len` and `contains` submodules.
    ModuleSpec {
        name: "len",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/array/len.hnl")),
        funcs: &["len"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "contains",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/array/contains.hnl")),
        funcs: &["contains"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── stats (former plugin-lib-stats) — pure handl ───────────────────
    ModuleSpec {
        name: "stats",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-stats/src/index.hnl")),
        funcs: &["sum", "avg", "product", "min", "max", "median", "stddev"],
        ops: &[],
        values: &[],
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
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-hello/src/index.hnl")),
        funcs: &["greet"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── result (former plugin-lib-result) — pure handl over `__tag__` ──
    // `result.ok`/`result.err` mint `{"__tag__": "ok"/"err", "value": _0}`
    // records via `hash_map.set` (the `tag.tag` producer was deleted —
    // ADR-0152 §Migration), so the `hash_map` module must resolve through
    // the ambient [`ModuleResolver`]. The injected global table lookup
    // resolves it through the resolver passed to `module_with`
    // (e.g. kit-canonical's `StdlibResolver`).
    ModuleSpec {
        name: "result",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-result/src/index.hnl")),
        funcs: &["Result", "ok", "err"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── obj (record introspection) ────────────────────────────────────
    // `obj.keys`/`obj.values`/`obj.entries` are the object-field
    // enumeration primitives (gap #3); used by `hash_map` and other
    // record ops.
    ModuleSpec {
        name: "obj",
        hnl: handl_lib_obj::OBJ_HNL,
        funcs: &[],
        ops: handl_lib_obj::OBJ_OPS,
        values: &[],
        registry: Some(handl_lib_obj::obj_op_registry),
    },
    // ── error (ADR-0092 gap (a)) — the error-raising primitive ─────────
    // `error.raise` is an irreducible atom: aborting evaluation cannot be
    // expressed over the eval vocabulary, so the module is ops-only. Any
    // `.hnl` module that needs to fail loudly resolves `error` through the
    // global lookup (or registers the module in its ambient chain) and calls
    // `error.raise(msg)`.
    ModuleSpec {
        name: "error",
        hnl: handl_lib_error::ERROR_HNL,
        funcs: &[],
        ops: handl_lib_error::ERROR_OPS,
        values: &[],
        registry: Some(handl_lib_error::error_op_registry),
    },
    // ── arrow_iter (former plugin-lib-arrow-iter) ──────────────────────
    // Declaration-only module: the `while`/`for`/`fix` derived forms are
    // `Term::Loop` descriptions built host-side (see
    // [`crate::arrow_iter`]); the composite `*_step` ops are interpreted
    // by the v2 engine, so the module exposes no functions or ops.
    ModuleSpec {
        name: "arrow_iter",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-arrow-iter/src/index.hnl")),
        funcs: &[],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── prim (former plugin-lib-prim) ──────────────────────────────────
    // Pure-handl record: `add`/`alloc`/`or` over builtins/`math`,
    // `compare` over `cmp.compare` to preserve exact total-ordering.
    ModuleSpec {
        name: "prim",
        hnl: handl_lib_prim_handl::PRIM_HANDL_HNL,
        funcs: &["add", "alloc", "or", "compare"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── archive / archive_tar (former plugin-lib-archive, plugin-lib-archive-tar)
    // The `archive` module is caps-only (no callable ops — it's a trait
    // abstraction). The `tar` module provides `tar.pack` and `tar.unpack` via
    // the consolidated Rust `Function` impls in `ops::tar`.
    ModuleSpec {
        name: "archive",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-archive/src/index.hnl")),
        funcs: &[],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "tar",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-tar/src/index.hnl")),
        funcs: &["pack", "unpack"],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── intl_c — the `intl` contract implemented over the system ICU4C ─
    // library via `ffi`. Pure-handl module; `available`/`icu_version` report
    // probe results so callers can gate on host ICU presence.
    ModuleSpec {
        name: "intl_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "compare_strings",
            "format_number",
            "format_number_to_parts",
            "select_plural",
            "format_list",
            "format_relative_time",
            "format_date",
            "format_date_to_parts",
            "format_duration",
            "display_name",
            "segment",
            "locale",
            "get_canonical_locales",
            "supported_locales_of",
        ],
        ops: &[],
        values: &["available", "icu_version"],
        registry: None,
    },
    // ── unicode_c — the `unicode` contract implemented over the system
    // ICU4C common library via `ffi` (`ubrk_*` UAX #29 grapheme
    // segmentation, `unorm2_*` NFC/NFD, `u_strFoldCase`, `u_hasBinaryProperty`
    // XID checks, EAST_ASIAN_WIDTH display width, UTF-16 marshalling,
    // UErrorCode readbacks). Pure-handl module; `available`/`icu_version`
    // report probe results so callers can gate on host ICU presence.
    ModuleSpec {
        name: "unicode_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-unicode-c/src/index.hnl")),
        funcs: &[
            "code_points",
            "from_code_points",
            "is_ascii",
            "code_point_at",
            "decode",
            "is_valid_identifier",
            "is_xid_start",
            "is_xid_continue",
            "graphemes",
            "grapheme_count",
            "width",
            "casefold",
            "nfc",
            "nfd",
        ],
        ops: &[],
        values: &["available", "icu_version"],
        registry: None,
    },
    // ── icu_c_* — sdkgen-generated ICU4C adapter modules (i18n) ────────
    // Table-only modules: `intl_c` reaches them via
    // `from "../generated/icu-c-linux-x86_64/<mod>_c" import * as <mod>`,
    // which basename-normalizes to these `*_c` names in the module table.
    // Not contributed by any package — they are intl_c's internal FFI
    // surface. Each adapter's own `from "../icu/<mod>" import` resolves
    // the bare-named port spec row below.
    ModuleSpec {
        name: "ucol_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "ucol_close_78",
            "ucol_countAvailable_78",
            "ucol_equal_78",
            "ucol_getAttribute_78",
            "ucol_getAvailable_78",
            "ucol_getDisplayName_78",
            "ucol_getLocaleByType_78",
            "ucol_getSortKey_78",
            "ucol_getStrength_78",
            "ucol_greater_78",
            "ucol_greaterOrEqual_78",
            "ucol_open_78",
            "ucol_openRules_78",
            "ucol_setAttribute_78",
            "ucol_setStrength_78",
            "ucol_strcoll_78",
            "ucol_strcollUTF8_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "unum_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "unum_close_78",
            "unum_format_78",
            "unum_formatDecimal_78",
            "unum_formatDouble_78",
            "unum_formatDoubleForFields_78",
            "unum_formatInt64_78",
            "unum_getAttribute_78",
            "unum_getLocaleByType_78",
            "unum_getSymbol_78",
            "unum_open_78",
            "unum_setAttribute_78",
            "unum_setSymbol_78",
            "unum_setTextAttribute_78",
            "unum_toPattern_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "upluralrules_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "uplrules_close_78",
            "uplrules_open_78",
            "uplrules_openForType_78",
            "uplrules_select_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ureldatefmt_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "ureldatefmt_close_78",
            "ureldatefmt_format_78",
            "ureldatefmt_formatNumeric_78",
            "ureldatefmt_open_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "udat_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "udat_close_78",
            "udat_format_78",
            "udat_formatForFields_78",
            "udat_open_78",
            "udat_parse_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ufieldpositer_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "ufieldpositer_close_78",
            "ufieldpositer_next_78",
            "ufieldpositer_open_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "unumberformatter_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "unumf_close_78",
            "unumf_formatDouble_78",
            "unumf_openForSkeletonAndLocale_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uformattednumber_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "unumf_closeResult_78",
            "unumf_openResult_78",
            "unumf_resultToString_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── icu_uc_*_c — sdkgen-generated ICU4C adapter modules (common) ──
    ModuleSpec {
        name: "ubrk_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "ubrk_close_78",
            "ubrk_countAvailable_78",
            "ubrk_current_78",
            "ubrk_first_78",
            "ubrk_getAvailable_78",
            "ubrk_getRuleStatus_78",
            "ubrk_last_78",
            "ubrk_next_78",
            "ubrk_open_78",
            "ubrk_previous_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uloc_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "uloc_countAvailable_78",
            "uloc_forLanguageTag_78",
            "uloc_getAvailable_78",
            "uloc_getCountry_78",
            "uloc_getLanguage_78",
            "uloc_getName_78",
            "uloc_getScript_78",
            "uloc_getVariant_78",
            "uloc_toLanguageTag_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uldnames_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "uldn_close_78",
            "uldn_keyDisplayName_78",
            "uldn_keyValueDisplayName_78",
            "uldn_languageDisplayName_78",
            "uldn_localeDisplayName_78",
            "uldn_open_78",
            "uldn_regionDisplayName_78",
            "uldn_scriptDisplayName_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ures_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "ures_close_78",
            "ures_getByKey_78",
            "ures_getUTF8String_78",
            "ures_open_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    // ── icu_uc_*_c — Unicode-data adapters used by `unicode_c` ────────
    ModuleSpec {
        name: "unorm2_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "unorm2_getNFCInstance_78",
            "unorm2_getNFDInstance_78",
            "unorm2_normalize_78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ustring_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &["u_strFoldCase_78"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uchar_c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "u_charType_78",
            "u_getIntPropertyValue_78",
            "u_hasBinaryProperty_78",
        ],
        ops: &[],
        values: &[
            "UCHAR_EAST_ASIAN_WIDTH",
            "UCHAR_XID_CONTINUE",
            "UCHAR_XID_START",
            "U_CONTROL_CHAR",
            "U_ENCLOSING_MARK",
            "U_EA_FULLWIDTH",
            "U_EA_WIDE",
            "U_FORMAT_CHAR",
            "U_LINE_SEPARATOR",
            "U_NON_SPACING_MARK",
            "U_PARAGRAPH_SEPARATOR",
        ],
        registry: None,
    },
    // ── icu_* — sdkgen-generated ICU4C port specs (contract) ──────────
    // The `*_c` adapters' `from "../<lib>/<mod>" import { <Sig>*78 }`
    // lines basename-resolve to these rows (the port keeps the bare
    // name, the impl the `_c` suffix — the `uchar` self-import split).
    //
    // Export names ride in `funcs`, not `values`: the port's exports are
    // contract-level names — `export Sig78: (…) ~[1]~> Result<…, Errno>`
    // signature declarations (which erase to `null` under Agda-style
    // signature erasure) and `{Port}Result` record shapes — bound only
    // to satisfy `export_field` projection at the adapter's import line;
    // the adapter references them solely in type-annotation position,
    // which `transform_export_to_object` strips before eval, so the
    // wrapper-arrow binding is never applied. `values:` would instead
    // evaluate the port source per name — which fails on
    // `from "ffi" import { Errno }` (`ffi` resolves through the host
    // module chain, not the package table) — leaving the export missing
    // and the adapter import erroring.
    ModuleSpec {
        name: "ubrk",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UbrkOpen78Result",
            "UbrkOpen78",
            "UbrkClose78",
            "UbrkCurrent78",
            "UbrkNext78",
            "UbrkPrevious78",
            "UbrkFirst78",
            "UbrkLast78",
            "UbrkGetAvailable78",
            "UbrkCountAvailable78",
            "UbrkGetRuleStatus78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uchar",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UCHAR_XID_CONTINUE",
            "UCHAR_XID_START",
            "UCHAR_EAST_ASIAN_WIDTH",
            "U_NON_SPACING_MARK",
            "U_ENCLOSING_MARK",
            "U_LINE_SEPARATOR",
            "U_PARAGRAPH_SEPARATOR",
            "U_CONTROL_CHAR",
            "U_FORMAT_CHAR",
            "U_EA_FULLWIDTH",
            "U_EA_WIDE",
            "UHasBinaryProperty78",
            "UGetIntPropertyValue78",
            "UCharType78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uldnames",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UldnOpen78Result",
            "UldnOpen78",
            "UldnClose78",
            "UldnLocaleDisplayName78Result",
            "UldnLocaleDisplayName78",
            "UldnLanguageDisplayName78Result",
            "UldnLanguageDisplayName78",
            "UldnScriptDisplayName78Result",
            "UldnScriptDisplayName78",
            "UldnRegionDisplayName78Result",
            "UldnRegionDisplayName78",
            "UldnKeyDisplayName78Result",
            "UldnKeyDisplayName78",
            "UldnKeyValueDisplayName78Result",
            "UldnKeyValueDisplayName78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uloc",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UlocGetLanguage78Result",
            "UlocGetLanguage78",
            "UlocGetScript78Result",
            "UlocGetScript78",
            "UlocGetCountry78Result",
            "UlocGetCountry78",
            "UlocGetVariant78Result",
            "UlocGetVariant78",
            "UlocGetName78Result",
            "UlocGetName78",
            "UlocCanonicalize78Result",
            "UlocCanonicalize78",
            "UlocGetAvailable78",
            "UlocCountAvailable78",
            "UlocGetParent78Result",
            "UlocGetParent78",
            "UlocForLanguageTag78Result",
            "UlocForLanguageTag78",
            "UlocToLanguageTag78Result",
            "UlocToLanguageTag78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "unorm2",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "Unorm2GetNfcInstance78Result",
            "Unorm2GetNfcInstance78",
            "Unorm2GetNfdInstance78Result",
            "Unorm2GetNfdInstance78",
            "Unorm2Normalize78Result",
            "Unorm2Normalize78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ures",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UresOpen78Result",
            "UresOpen78",
            "UresOpenDirect78Result",
            "UresOpenDirect78",
            "UresClose78",
            "UresGetLocaleByType78Result",
            "UresGetLocaleByType78",
            "UresGetUtf8String78Result",
            "UresGetUtf8String78",
            "UresGetByKey78Result",
            "UresGetByKey78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ustring",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &["UStrFoldCase78Result", "UStrFoldCase78"],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ucol",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UcolOpen78Result",
            "UcolOpen78",
            "UcolOpenRules78Result",
            "UcolOpenRules78",
            "UcolClose78",
            "UcolStrcoll78",
            "UcolStrcollUtf878Result",
            "UcolStrcollUtf878",
            "UcolGreater78",
            "UcolGreaterOrEqual78",
            "UcolEqual78",
            "UcolGetStrength78",
            "UcolSetStrength78",
            "UcolGetDisplayName78Result",
            "UcolGetDisplayName78",
            "UcolGetAvailable78",
            "UcolCountAvailable78",
            "UcolGetSortKey78Result",
            "UcolGetSortKey78",
            "UcolSetAttribute78Result",
            "UcolSetAttribute78",
            "UcolGetAttribute78Result",
            "UcolGetAttribute78",
            "UcolGetLocaleByType78Result",
            "UcolGetLocaleByType78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "udat",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UdatOpen78Result",
            "UdatOpen78",
            "UdatClose78",
            "UdatFormat78Result",
            "UdatFormat78",
            "UdatFormatForFields78Result",
            "UdatFormatForFields78",
            "UdatParse78Result",
            "UdatParse78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ufieldpositer",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UfieldpositerOpen78Result",
            "UfieldpositerOpen78",
            "UfieldpositerClose78",
            "UfieldpositerNext78Result",
            "UfieldpositerNext78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "uformattednumber",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UnumfOpenResult78Result",
            "UnumfOpenResult78",
            "UnumfResultToString78Result",
            "UnumfResultToString78",
            "UnumfCloseResult78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "unumberformatter",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UnumfOpenForSkeletonAndLocale78Result",
            "UnumfOpenForSkeletonAndLocale78",
            "UnumfFormatDouble78Result",
            "UnumfFormatDouble78",
            "UnumfClose78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "unum",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UnumOpen78Result",
            "UnumOpen78",
            "UnumClose78",
            "UnumFormat78Result",
            "UnumFormat78",
            "UnumFormatInt6478Result",
            "UnumFormatInt6478",
            "UnumFormatDouble78Result",
            "UnumFormatDouble78",
            "UnumFormatDoubleForFields78Result",
            "UnumFormatDoubleForFields78",
            "UnumFormatDecimal78Result",
            "UnumFormatDecimal78",
            "UnumGetAttribute78Result",
            "UnumGetAttribute78",
            "UnumSetAttribute78Result",
            "UnumSetAttribute78",
            "UnumSetTextAttribute78Result",
            "UnumSetTextAttribute78",
            "UnumToPattern78Result",
            "UnumToPattern78",
            "UnumGetSymbol78Result",
            "UnumGetSymbol78",
            "UnumSetSymbol78Result",
            "UnumSetSymbol78",
            "UnumGetLocaleByType78Result",
            "UnumGetLocaleByType78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "upluralrules",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UplrulesOpen78Result",
            "UplrulesOpen78",
            "UplrulesOpenForType78Result",
            "UplrulesOpenForType78",
            "UplrulesClose78",
            "UplrulesSelect78Result",
            "UplrulesSelect78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
    ModuleSpec {
        name: "ureldatefmt",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl-c/src/index.hnl")),
        funcs: &[
            "UreldatefmtOpen78Result",
            "UreldatefmtOpen78",
            "UreldatefmtClose78",
            "UreldatefmtFormatNumeric78Result",
            "UreldatefmtFormatNumeric78",
            "UreldatefmtFormat78Result",
            "UreldatefmtFormat78",
        ],
        ops: &[],
        values: &[],
        registry: None,
    },
];
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
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-utils/src/utils.hnl")),
        modules: &[], // resolved via UTILS_MODULES (see package_modules)
    },
    PackageSpec {
        lib_name: "cmp",
        plugin_id: "lib.cmp",
        plugin_name: "lib-cmp",
        hnl: handl_lib_cmp::CMP_HNL,
        modules: &[],
    },
    PackageSpec {
        lib_name: "collections",
        plugin_id: "lib.collections",
        plugin_name: "lib-collections",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-collections/src/collections.hnl")),
        modules: &[], // resolved via COLLECTIONS_MODULES (see package_module_names)
    },
    PackageSpec {
        lib_name: "stats",
        plugin_id: "lib.stats",
        plugin_name: "lib-stats",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-stats/src/index.hnl")),
        modules: &[],
    },
    // Single-module packages also usable standalone (idempotent with the
    // `utils` aggregate, which contributes the same modules).
    PackageSpec {
        lib_name: "crypto",
        plugin_id: "lib.crypto",
        plugin_name: "lib-crypto",
        hnl: handl_plugin_lib_crypto::CRYPTO_HNL,
        modules: &[],
    },
    PackageSpec {
        lib_name: "json",
        plugin_id: "lib.json",
        plugin_name: "lib-json",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-json/src/index.hnl")),
        modules: &[],
    },
    PackageSpec {
        lib_name: "base64",
        plugin_id: "lib.base64",
        plugin_name: "lib-base64",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-base64/src/index.hnl")),
        modules: &[],
    },
    PackageSpec {
        lib_name: "hex",
        plugin_id: "lib.hex",
        plugin_name: "lib-hex",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-hex/src/index.hnl")),
        modules: &[],
    },
    PackageSpec {
        lib_name: "url_rfc",
        plugin_id: "lib.url-rfc",
        plugin_name: "lib-url-rfc",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-url-rfc/src/index.hnl")),
        modules: &[],
    },
    PackageSpec {
        lib_name: "url_whatwg",
        plugin_id: "lib.url-whatwg",
        plugin_name: "lib-url-whatwg",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-url-whatwg/src/index.hnl")),
        modules: &[],
    },
    // ── hello / result / arrow_iter (former plugin-lib-{hello,result,
    // arrow-iter}) — pure-shell folds ─────────────────────────────────
    PackageSpec {
        lib_name: "hello",
        plugin_id: "lib.hello",
        plugin_name: "lib-hello",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-hello/src/index.hnl")),
        modules: &[],
    },
    // NOTE: the `result` module's `tag` dep is not in the table — a kit
    // consuming `library("result")` must supply a resolver fallback that
    // maps `"tag"` (see the `result` [`ModuleSpec`] row above).
    PackageSpec {
        lib_name: "result",
        plugin_id: "lib.result",
        plugin_name: "lib-result",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-result/src/index.hnl")),
        modules: &[],
    },
    PackageSpec {
        lib_name: "error",
        plugin_id: "lib.error",
        plugin_name: "lib-error",
        hnl: handl_lib_error::ERROR_HNL,
        modules: &[],
    },
    PackageSpec {
        lib_name: "bytes",
        plugin_id: "lib.bytes",
        plugin_name: "lib-bytes",
        hnl: handl_lib_bytes::BYTES_HNL,
        modules: &[],
    },
    PackageSpec {
        lib_name: "obj",
        plugin_id: "lib.obj",
        plugin_name: "lib-obj",
        hnl: handl_lib_obj::OBJ_HNL,
        modules: &[],
    },
    PackageSpec {
        lib_name: "arrow_iter",
        plugin_id: "lib.arrow-iter",
        plugin_name: "lib-arrow-iter",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-arrow-iter/src/index.hnl")),
        modules: &[],
    },
    PackageSpec {
        lib_name: "prim",
        plugin_id: "lib.prim",
        plugin_name: "lib-prim",
        hnl: handl_lib_prim::PRIM_HNL,
        modules: &[],
    },
    // ── archive-tar (former plugin-lib-archive-tar) ─────────────────────
    PackageSpec {
        lib_name: "archive-tar",
        plugin_id: "lib.archive-tar",
        plugin_name: "lib-archive-tar",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-tar/src/index.hnl")),
        modules: &[], // resolved via ARCHIVE_TAR_MODULES (see package_module_names)
    },
    // ── datetime (former plugin-lib-datetime) — pure handl ─────────────
    // The `datetime` module is a self-contained pure-handl record (see its
    // `ModuleSpec` row above); the package contributes just that module.
    PackageSpec {
        lib_name: "datetime",
        plugin_id: "lib.datetime",
        plugin_name: "lib-datetime",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-datetime/src/index.hnl")),
        modules: &[],
    },
    // ── intl_c — `intl` contract backend over system ICU4C via `ffi` ────
    // The package's capability surface is the `intl` contract itself; the
    // `intl_c` module is the implementation. Mirrors the `archive`/`tar`
    // contract/impl split.
    PackageSpec {
        lib_name: "intl_c",
        plugin_id: "lib.intl-c",
        plugin_name: "lib-intl-c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-intl/src/index.hnl")),
        modules: &[],
    },
    // ── unicode_c — `unicode` contract backend over system ICU4C via `ffi` ─
    // The package's capability surface is the `unicode` contract itself; the
    // `unicode_c` module is the implementation. Mirrors the `intl`/`intl_c`
    // contract/impl split.
    PackageSpec {
        lib_name: "unicode_c",
        plugin_id: "lib.unicode-c",
        plugin_name: "lib-unicode-c",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-unicode/src/index.hnl")),
        modules: &[],
    },
    // ── ip (former plugin-lib-ip) — native ops in `lib-ip` ───────────────
    // The `ip` module dispatches across the `ip` / `cidr` / `socket`
    // prefixes with colliding local op names (`ip.parse` vs
    // `ip.cidr.parse`), so it is contributed as the `IpModule` aggregate
    // rather than table-built sub-modules (see `package_module_names`).
    PackageSpec {
        lib_name: "ip",
        plugin_id: "lib.ip",
        plugin_name: "lib-ip",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-ip/src/index.hnl")),
        modules: &[],
    },
    // ── char (former plugin-lib-char) — contract only ────────────────────
    // `char` is an axiom contract supplied by the embedding host; the
    // package row exposes its capability surface without contributing a
    // table module (a table `char` would shadow the host's native module).
    PackageSpec {
        lib_name: "char",
        plugin_id: "lib.char",
        plugin_name: "lib-char",
        hnl: include_str!(concat!(env!("OUT_DIR"), "/lib-char/src/index.hnl")),
        modules: &[],
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
    if spec.lib_name == "ip" {
        // `ip` is contributed as the `IpModule` aggregate (below), not as
        // table-built sub-modules: its ops share last-segment names
        // (`parse`, `format`) across the `ip`/`cidr`/`socket` prefixes.
        return Vec::new();
    }
    if spec.lib_name == "char" {
        // `char` is an axiom contract provided by the embedding host; the
        // package row carries its caps but contributes no table module.
        return Vec::new();
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
/// asks the resolver for the target module's bindings record. Since ADR-0155
/// Phase 1 the payload is a **callable record** — one `wrap_dep_fn` wrapper
/// arrow per function export (plus value exports as values) via
/// [`handl_plugin_lib_handl::module_bindings_tagged`]. Applying a binding
/// (`m.f(x)`, or a bare named import `f(x)`) routes the arrow through `!App`
/// to the same `call_host` leaf a `"<module>.<field>"` string callee
/// dispatched to — resolving through the dependency chain or the injected
/// global table lookup. This keeps `from "x" import * as m` (`m.f`) working
/// *and* enables named imports (`from "./x.hnl" import f` -> bare `f(...)`),
/// now with real callable values (both encodings apply — the string arm
/// stays as fallback for hosts that emit it).
///
/// `path` is normalized to a module name ([`normalize_module_name`]) and the
/// module is built from the table -- cached, so repeated imports of the same
/// module never re-evaluate its source.
pub struct TableImportResolver {
    cache: Mutex<HashMap<String, Arc<serde_json::Value>>>,
    resolver: Arc<dyn ModuleResolver>,
}

/// Shared same-thread self-import guard (keyed by `(thread, name)`): a spec
/// whose source basename-normalizes to its own name recurses through
/// eval → import → resolve → bindings → eval forever, and the loop
/// crosses *instances* — every `build_module` wires a fresh
/// `TableImportResolver`, so a per-instance set never sees the re-entry.
/// (The historical case: `icu-uc-c-linux-x86_64/uchar.hnl` imported
/// `../icu-uc/uchar` — basename `uchar` resolving back to itself. The
/// port/adapter `_c` split renamed the adapter to `uchar_c`, so its
/// import now lands on the `uchar` *port* row; the guard still covers
/// any future same-basename spec.)
/// A static is shared by every resolver; a different thread resolving the
/// same name is a concurrent build, not a cycle (same rule as
/// `TableModuleLookup::in_progress`).
static TABLE_IMPORT_IN_PROGRESS: std::sync::LazyLock<
    Mutex<HashSet<(std::thread::ThreadId, String)>>,
> = std::sync::LazyLock::new(|| Mutex::new(HashSet::new()));

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
    fn resolve_import(&self, path: &str) -> Result<serde_json::Value, handl_language::LangError> {
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
        let key = (std::thread::current().id(), name.clone());
        {
            let mut in_progress = TABLE_IMPORT_IN_PROGRESS.lock().map_err(|_| {
                handl_language::LangError::engine("import: resolver lock poisoned")
            })?;
            if !in_progress.insert(key.clone()) {
                return Err(handl_language::LangError::engine(format!(
                    "import: module `{name}` cannot import itself (self-import cycle at `{path}`)"
                )));
            }
        }
        let result = (|spec| {
            let module = build_module(spec, Arc::clone(&self.resolver));
            // ADR-0155 Phase 1: the record fields are wrapper arrows (callable
            // values), encoded through the `Value` boundary as `{"__arrow__":…}`
            // and decoded back by `TaggedValue::from` on the tagged fast path.
            let value = Arc::new(handl_value::tagged_to_value(
                &handl_plugin_lib_handl::module_bindings_tagged(&module),
            ));
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(name, Arc::clone(&value));
            }
            Ok((*value).clone())
        })(spec);
        if let Ok(mut in_progress) = TABLE_IMPORT_IN_PROGRESS.lock() {
            in_progress.remove(&key);
        }
        result
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
    // Keyed by (thread, name): `in_progress` is a same-thread *recursion*
    // guard — a module build re-entering `resolve_fallback` for its own name
    // on this thread is a cycle. A *different* thread resolving the same name
    // is a concurrent build, not a cycle: it must proceed (the `fallback`
    // cache's `or_insert` dedups the result). Keying by name alone raced:
    // the second thread misread the first thread's in-flight build as a
    // cycle and `lookup` returned `None` — e.g. `entropy.next_u32`'s
    // `ffi.call` flaking to "function 'ffi.call' not found" under parallel
    // first-use (kit-prelude `rand_c`).
    in_progress: Mutex<HashSet<(std::thread::ThreadId, String)>>,
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
        let key = (std::thread::current().id(), normalized.to_string());
        {
            let mut in_progress = self.in_progress.lock().ok()?;
            if !in_progress.insert(key.clone()) {
                // Same thread already building this module further up the
                // stack: a cycle.
                return None;
            }
        }
        let built = self.resolver.resolve(normalized);
        if let Ok(mut in_progress) = self.in_progress.lock() {
            in_progress.remove(&key);
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

// ---------------------------------------------------------------------------
// Default env (ADR-0155 Phase 3) — the ambient table as explicit env globals
// ---------------------------------------------------------------------------

/// The function fields of a table [`ModuleSpec`] as an env-record object:
/// one [`wrap_dep_fn`](handl_plugin_lib_handl::wrap_dep_fn) wrapper arrow
/// per `funcs` name and per `ops` local name (the segment after the last
/// `.`), keyed by field name.
///
/// Pure data — minted from the spec row, no module construction and no
/// evaluation. Applying a field re-resolves `"spec.field"` through the
/// *calling* module's `ModuleLookup` leaf (`callv` → `call_host`), so the
/// record is portable across resolvers: the leaf stays `resolver`-keyed
/// even though the record is shared.
fn spec_arrow_record(spec: &ModuleSpec) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for &fname in spec.funcs {
        obj.insert(
            fname.to_string(),
            handl_language::Value::from(handl_plugin_lib_handl::wrap_dep_fn(&format!(
                "{}.{fname}",
                spec.name
            ))),
        );
    }
    for &op_name in spec.ops {
        let local = op_name.rsplit('.').next().unwrap_or(op_name);
        obj.entry(local.to_string()).or_insert_with(|| {
            handl_language::Value::from(handl_plugin_lib_handl::wrap_dep_fn(&format!(
                "{}.{local}",
                spec.name
            )))
        });
    }
    serde_json::Value::Object(obj)
}

/// A [`DefaultEnv`](handl_plugin_lib_handl::DefaultEnv) provider that
/// binds every [`MODULES`] table name to its bindings record — the
/// ambient std surface made explicit (ADR-0155 Phase 3).
///
/// Scope-first layering falls out of `build_runtime`'s merge: locals and
/// `import` splices resolve first, declared deps next, these defaults
/// last — a dep named `math` shadows the table's `math`, and a `.hnl`
/// source that never declared `str` still finds `str.trim` bound.
///
/// The records are **pure data** minted from the spec rows
/// ([`spec_arrow_record`]) — no module construction, no evaluation, no
/// resolver. Value exports (`spec.values`, e.g. `bool`'s `true`/`false`)
/// are deliberately *not* projected: projecting them would evaluate the
/// module's `.hnl` record during env materialization — an import cycle
/// in any row's source would recurse (as `uchar`'s self-import did
/// before the `*_c` port/adapter split). Ambient
/// `mod.value` reads were never usable anyway — an unbound `bool.true`
/// evaluated to the *string* `"bool.true"`, not `true`; via the env
/// record it now reads `Null`. `from "bool" import true` remains the
/// real-value binding path.
///
/// The residual ambient: non-table names the module's resolver knows
/// (`char`, `tag`, `sym`, `ffi`, kit aliases like `fn`) are *not* bound
/// here — they keep resolving as unbound→string callees through the
/// `global_lookup` leaf, as do the `field_access`/`fn.fanout_fn`
/// builtins. A dotted callee matching none of these layers fails at
/// `call_host` (`function '<name>' not found`).
pub fn std_default_env() -> handl_plugin_lib_handl::DefaultEnv {
    Arc::new(|| {
        MODULES
            .iter()
            .map(|spec| (spec.name.to_string(), spec_arrow_record(spec)))
            .collect()
    })
}

/// `build_module` minus the Phase-3 default env — the shared base: the
/// provider itself is pure data, but hosts constructing a module without
/// ambient bindings get the same table wiring.
fn build_module_shallow(spec: &ModuleSpec, resolver: Arc<dyn ModuleResolver>) -> HandlModule {
    let lookup = Arc::new(TableModuleLookup::new(Arc::clone(&resolver)));
    let mut module = HandlModule::new(spec.name, spec.hnl, spec.funcs)
        .with_value_names(spec.values)
        .with_import_resolver(Arc::new(TableImportResolver::with_resolver(Arc::clone(
            &resolver,
        ))))
        .with_global_lookup(lookup);
    if let Some(registry) = spec.registry {
        module = module.with_ops(spec.ops, Arc::new(registry()));
    }
    module
}

/// Build a [`HandlModule`] from a [`ModuleSpec`], wiring op dispatch and an
/// injected global [`TableModuleLookup`]. Resolution is global, so the module
/// reaches every other table module (and the resolver's non-table modules)
/// through the lookup.
///
/// ADR-0155 Phase 3: the module table is *also* bound as env globals via
/// [`std_default_env`] — table names resolve scope-first (as records of
/// wrapper arrows, shadowed by declared deps and imports) while the
/// lookup remains the `call_host` leaf the wrappers re-resolve through,
/// plus the residual ambient for non-table names.
pub fn build_module(spec: &ModuleSpec, resolver: Arc<dyn ModuleResolver>) -> HandlModule {
    build_module_shallow(spec, resolver).with_default_env(std_default_env())
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
///
/// The build runs *outside* the lock: `HandlModule::new`'s `analyze_source`
/// can run a nested eval that re-enters this cache — holding the lock
/// across `module()` would deadlock on the same thread.
pub fn module_ref(name: &str) -> Option<&'static HandlModule> {
    module_spec(name)?;
    if let Some(m) = module_cache().lock().ok()?.get(name) {
        return Some(*m);
    }
    // `with_loader_disabled`: `HandlModule::new`'s `analyze_source` would
    // otherwise run `loader.analyze` — a nested eval that can re-enter
    // this cache (miss → build → analyze → …) — unbounded recursion.
    let built: &'static HandlModule =
        Box::leak(Box::new(handl_plugin_lib_handl::with_loader_disabled(
            || module(name).expect("module_spec checked above"),
        )));
    let mut cache = module_cache().lock().ok()?;
    Some(*cache.entry(name.to_string()).or_insert(built))
}

/// Env-keyed sibling of [`module_cache`] (ADR-0156 follow-up): shared
/// table-module instances per `(name, env_id)`.
///
/// The name-keyed singleton above is untouched — through [`module_ref`]
/// one name still means one env there; this table is additive, so one
/// name can mean several envs.
fn env_module_cache() -> &'static Mutex<HashMap<(String, usize), Arc<HandlModule>>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, usize), Arc<HandlModule>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get a shared [`Arc<HandlModule>`] for the table module `name`
/// instantiated under `env` (ADR-0156 follow-up).
///
/// Env-aware sibling of [`module_ref`]: the *base* still comes from the
/// name-keyed singleton (the source analysis runs once per name and is
/// reused by every env), and the instance —
/// [`HandlModule::instantiate`]d under `env`, which replaces the table's
/// global lookup/import resolver with the env's — is cached by
/// `(name, env_id)`. Clones of one env share a single instance; distinct
/// envs (distinct [`ModuleEnv::env_id`]s) get distinct `module_id`s.
///
/// A useful env shape is `base.env().with_dependency(extra)`: the
/// snapshot carries the table wiring (global lookup + import resolver)
/// and the layered dep shadows the table for that instance (the
/// dependency registry is consulted before the global lookup). The
/// `&'static` [`module_ref`] singleton keeps its single-env behavior
/// unchanged.
pub fn module_ref_with_env(name: &str, env: &ModuleEnv) -> Option<Arc<HandlModule>> {
    let key = (name.to_string(), env.env_id());
    if let Ok(cache) = env_module_cache().lock() {
        if let Some(m) = cache.get(&key) {
            return Some(Arc::clone(m));
        }
    }
    // `instantiate` shares the base's prepared source (no re-analysis),
    // so it runs outside the lock without the loader recursion concern
    // documented on `module_ref`.
    let base = module_ref(name)?;
    let instance = Arc::new(base.instantiate(env.clone()));
    let mut cache = env_module_cache().lock().ok()?;
    Some(Arc::clone(cache.entry(key).or_insert(instance)))
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
        assert_eq!(
            normalize_module_name("collections/array/reverse.hnl"),
            "reverse"
        );
        // Private module name.
        assert_eq!(normalize_module_name("./_reflect.hnl"), "_reflect");
    }
}
