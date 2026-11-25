//! `handl-kit-stdlib` — the general-purpose library kit.
//!
//! Extracted from `handl-kit-canonical`: the `.hnl`-backed data-format and
//! utility modules (`json`, `base64`, `hex`, `tar`, `url_rfc`,
//! `url_whatwg`, `serialize`, `ip`, `kv_map`, `perlin`,
//! `simplex`) plus the `collections` aggregate, and the kit-loader
//! `LibraryRegistry` rows for the `stdlib`/`cli`/`hello` manifests.

use std::sync::Arc;

use handl_kit_core::{KitResolver, ModuleCtor, ModuleEntry};
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_stdlib::ops::collections::CollectionsModule;

// ---------------------------------------------------------------------------
// vendored .hnl sources (ADR-0160 P3) — build.rs copies each const-only
// lib-* repo into OUT_DIR; the consts below reproduce the retired
// `handl-lib-*` crates' HNL/FUNCS/SOURCES tables verbatim.
// ---------------------------------------------------------------------------

// base64 — pure `.hnl` codec (T-044: crate dep dropped, pkg vendored).
const BASE64_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-base64/src/index.hnl"));
const BASE64_FUNCS: &[&str] = &["encode", "decode", "codec"];
const BASE64_SOURCES: &[(&str, &str, &[&str])] = &[
    (
        "encode",
        include_str!(concat!(env!("OUT_DIR"), "/lib-base64/src/encode.hnl")),
        &["encode"],
    ),
    (
        "decode",
        include_str!(concat!(env!("OUT_DIR"), "/lib-base64/src/decode.hnl")),
        &["decode"],
    ),
];

// hex — pure `.hnl` codec.
const HEX_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-hex/src/index.hnl"));
const HEX_FUNCS: &[&str] = &["encode", "decode", "codec"];
const HEX_SOURCES: &[(&str, &str, &[&str])] = &[
    (
        "encode",
        include_str!(concat!(env!("OUT_DIR"), "/lib-hex/src/encode.hnl")),
        &["encode"],
    ),
    (
        "decode",
        include_str!(concat!(env!("OUT_DIR"), "/lib-hex/src/decode.hnl")),
        &["decode"],
    ),
];

// serialize — format-agnostic serialization port (`.hnl` module surface).
const SERIALIZE_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-serialize/src/index.hnl"));
const SERIALIZE_FUNCS: &[&str] = &["serialize", "deserialize", "toml", "yaml"];
const SERIALIZE_SOURCES: &[(&str, &str, &[&str])] = &[(
    "serialize",
    include_str!(concat!(env!("OUT_DIR"), "/lib-serialize/src/serialize.hnl")),
    &["fmt_name", "serialize", "deserialize", "toml", "yaml"],
)];

// ---------------------------------------------------------------------------
// perlin module (pure Handl — ADR-0109)
// ---------------------------------------------------------------------------

const PERLIN_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-perlin/src/perlin.hnl"));
const PERLIN_FUNCS: &[&str] = &["perlin_1d", "perlin_2d", "perlin_3d"];

// ---------------------------------------------------------------------------
// simplex module (pure Handl — ADR-0109)
// ---------------------------------------------------------------------------

const SIMPLEX_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-simplex/src/simplex.hnl"));
const SIMPLEX_FUNCS: &[&str] = &["simplex_2d", "simplex_3d"];

fn build_perlin_module() -> HandlModule {
    HandlModule::new("perlin", PERLIN_HNL, PERLIN_FUNCS).with_global_lookup(std::sync::Arc::new(
        stdlib::TableModuleLookup::new(std::sync::Arc::new(KitResolver)),
    ))
}

fn build_simplex_module() -> HandlModule {
    HandlModule::new("simplex", SIMPLEX_HNL, SIMPLEX_FUNCS).with_global_lookup(std::sync::Arc::new(
        stdlib::TableModuleLookup::new(std::sync::Arc::new(KitResolver)),
    ))
}

const JSON_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-json/src/index.hnl"));
const JSON_FUNCS: &[&str] = &["stringify", "parse", "json"];
const JSON_SOURCES: &[(&str, &str, &[&str])] = &[
    (
        "helpers",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-json/src/internal/helpers.hnl"
        )),
        &["ws_p", "num_p", "alpha_p", "slen", "take", "drop", "esc"],
    ),
    (
        "stringify",
        include_str!(concat!(env!("OUT_DIR"), "/lib-json/src/stringify.hnl")),
        &["stringify"],
    ),
    (
        "parse",
        include_str!(concat!(env!("OUT_DIR"), "/lib-json/src/parse.hnl")),
        &["parse"],
    ),
];

fn build_json_module() -> HandlModule {
    handl_plugin_lib_handl::handl_module_from_sources(
        "json",
        JSON_HNL,
        JSON_FUNCS,
        JSON_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}
fn build_base64_module() -> HandlModule {
    handl_plugin_lib_handl::handl_module_from_sources(
        "base64",
        BASE64_HNL,
        BASE64_FUNCS,
        BASE64_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}
fn build_hex_module() -> HandlModule {
    handl_plugin_lib_handl::handl_module_from_sources(
        "hex",
        HEX_HNL,
        HEX_FUNCS,
        HEX_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}
const IP_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-ip/src/index.hnl"));
const IP_FUNCS: &[&str] = &[
    "parse",
    "format",
    "is_loopback",
    "is_multicast",
    "is_private",
    "is_link_local",
    "is_unspecified",
    "cidr.parse",
    "cidr.format",
    "cidr.contains",
    "cidr.network",
    "cidr.broadcast",
    "socket.parse",
    "socket.format",
];
const IP_SOURCES: &[(&str, &str, &[&str])] = &[
    (
        "num",
        include_str!(concat!(env!("OUT_DIR"), "/lib-ip/src/internal/num.hnl")),
        &["dec_int", "int_dec"],
    ),
    (
        "ipv4",
        include_str!(concat!(env!("OUT_DIR"), "/lib-ip/src/ipv4.hnl")),
        &[
            "parse",
            "format",
            "is_loopback",
            "is_multicast",
            "is_private",
            "is_link_local",
            "is_unspecified",
            "parse_int",
            "format_int",
            "int_of",
        ],
    ),
    (
        "cidr",
        include_str!(concat!(env!("OUT_DIR"), "/lib-ip/src/cidr.hnl")),
        &[
            "cidr_parse",
            "cidr_format",
            "contains",
            "network",
            "broadcast",
        ],
    ),
    (
        "socket",
        include_str!(concat!(env!("OUT_DIR"), "/lib-ip/src/socket.hnl")),
        &["socket_parse", "socket_format"],
    ),
];

/// `lib-ip-c/atoms.yaml` (P-013 T-151) — the `ip.*` atom spec whose
/// `impl:` keys resolve against [`handl_plugin_lib_ip_c::ip_impls`].
const IP_C_ATOMS: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-ip-c/atoms.yaml"));

fn build_ip_module() -> HandlModule {
    // Spec-driven op registry (T-151): the 14 `ip.*` atoms bind the
    // native lib-ip-c impls (IPv4+IPv6/CIDR/socket). Op functions take
    // precedence over the `.hnl` record funcs of the same names at
    // `Module::function` dispatch, so `ip.*` wires hit the native ops
    // while the module's evaluated record stays the `.hnl` surface.
    let decls = stdlib::atoms::module_atoms(IP_C_ATOMS, "ip").expect("ip-c atoms spec");
    let registry = stdlib::atoms::registry_from_atoms_kernel(
        &decls,
        &handl_plugin_lib_ip_c::ip_impls(),
    )
    .expect("ip-c atoms registry");
    let op_names: Vec<&str> = decls.iter().map(|a| a.name.as_str()).collect();
    handl_plugin_lib_handl::handl_module_from_sources(
        "ip",
        IP_HNL,
        IP_FUNCS,
        IP_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_ops(&op_names, Arc::new(registry))
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}
const URL_RFC_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-url-rfc/src/index.hnl"));
const URL_RFC_FUNCS: &[&str] = &["parse", "url_rfc"];
const URL_RFC_SOURCES: &[(&str, &str, &[&str])] = &[
    (
        "slist",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-url-rfc/src/internal/slist.hnl"
        )),
        &["slen", "take", "drop", "s_slice", "s_len"],
    ),
    (
        "parse",
        include_str!(concat!(env!("OUT_DIR"), "/lib-url-rfc/src/parse.hnl")),
        &["parse"],
    ),
];

fn build_url_rfc_module() -> HandlModule {
    handl_plugin_lib_handl::handl_module_from_sources(
        "url_rfc",
        URL_RFC_HNL,
        URL_RFC_FUNCS,
        URL_RFC_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}
const URL_WHATWG_HNL: &str =
    include_str!(concat!(env!("OUT_DIR"), "/lib-url-whatwg/src/index.hnl"));
const URL_WHATWG_FUNCS: &[&str] = &["parse", "url_whatwg"];
const URL_WHATWG_SOURCES: &[(&str, &str, &[&str])] = &[
    (
        "slist",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-url-whatwg/src/internal/slist.hnl"
        )),
        &["slen", "take", "drop", "s_slice", "s_len"],
    ),
    (
        "parse",
        include_str!(concat!(env!("OUT_DIR"), "/lib-url-whatwg/src/parse.hnl")),
        &["parse"],
    ),
];

fn build_url_whatwg_module() -> HandlModule {
    handl_plugin_lib_handl::handl_module_from_sources(
        "url_whatwg",
        URL_WHATWG_HNL,
        URL_WHATWG_FUNCS,
        URL_WHATWG_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}
const TAR_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-tar/src/index.hnl"));
const TAR_FUNCS: &[&str] = &["pack", "unpack"];
const TAR_SOURCES: &[(&str, &str, &[&str])] = &[
    (
        "oct",
        include_str!(concat!(env!("OUT_DIR"), "/lib-tar/src/internal/oct.hnl")),
        &["oct_int", "int_oct", "octdx"],
    ),
    (
        "b64b",
        include_str!(concat!(env!("OUT_DIR"), "/lib-tar/src/internal/b64b.hnl")),
        &["b64_enc_bytes", "b64_dec_bytes"],
    ),
    (
        "headerb",
        include_str!(concat!(
            env!("OUT_DIR"),
            "/lib-tar/src/internal/headerb.hnl"
        )),
        &["header", "block"],
    ),
    (
        "pack",
        include_str!(concat!(env!("OUT_DIR"), "/lib-tar/src/pack.hnl")),
        &["pack"],
    ),
    (
        "unpack",
        include_str!(concat!(env!("OUT_DIR"), "/lib-tar/src/unpack.hnl")),
        &["unpack"],
    ),
];

fn build_tar_module() -> HandlModule {
    handl_plugin_lib_handl::handl_module_from_sources(
        "tar",
        TAR_HNL,
        TAR_FUNCS,
        TAR_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}
fn build_serialize_module() -> HandlModule {
    handl_plugin_lib_handl::handl_module_from_sources(
        "serialize",
        SERIALIZE_HNL,
        SERIALIZE_FUNCS,
        SERIALIZE_SOURCES,
        &KitResolver,
        "tree-walk",
    )
    .with_global_lookup(std::sync::Arc::new(stdlib::TableModuleLookup::new(
        std::sync::Arc::new(KitResolver),
    )))
}

// ---------------------------------------------------------------------------
// registration
// ---------------------------------------------------------------------------

/// The kit's module table: `.hnl` codec/utility modules plus `kv_map` and
/// the `collections` aggregate.
pub const MODULES: &[(&'static str, ModuleCtor)] = &[
    ("perlin", |_, e| {
        ModuleEntry::Handl(build_perlin_module().with_eval_engine(e))
    }),
    ("simplex", |_, e| {
        ModuleEntry::Handl(build_simplex_module().with_eval_engine(e))
    }),
    ("json", |_, e| {
        ModuleEntry::Handl(build_json_module().with_eval_engine(e))
    }),
    ("base64", |_, e| {
        ModuleEntry::Handl(build_base64_module().with_eval_engine(e))
    }),
    ("hex", |_, e| {
        ModuleEntry::Handl(build_hex_module().with_eval_engine(e))
    }),
    ("ip", |_, e| {
        ModuleEntry::Handl(build_ip_module().with_eval_engine(e))
    }),
    ("url_rfc", |_, e| {
        ModuleEntry::Handl(build_url_rfc_module().with_eval_engine(e))
    }),
    ("url_whatwg", |_, e| {
        ModuleEntry::Handl(build_url_whatwg_module().with_eval_engine(e))
    }),
    ("tar", |_, e| {
        ModuleEntry::Handl(build_tar_module().with_eval_engine(e))
    }),
    ("serialize", |_, e| {
        ModuleEntry::Handl(build_serialize_module().with_eval_engine(e))
    }),
    ("kv_map", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("kv_map", Arc::new(*r))
                .expect("kv_map")
                .with_eval_engine(e),
        )
    }),
    ("collections", |_, _| {
        ModuleEntry::Native(Arc::new(CollectionsModule::new(std::sync::Arc::new(
            stdlib::DEFAULT_RESOLVER.clone(),
        ))))
    }),
];

/// Register the stdlib kit's modules into the shared kit table.
/// `Once`-safe to call repeatedly.
pub fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| handl_kit_core::register_modules(MODULES));
}

// ---------------------------------------------------------------------------
// LibraryRegistry rows — the `stdlib`/`cli`/`hello` manifests' `modules:`
// names resolve through this table (ADR-0128 §0.4).
// ---------------------------------------------------------------------------

/// Populate `reg` with this kit's library-module constructors: the
/// table-driven libraries (`hello`, `utils`, `cmp`, `json`, `toml`,
/// `yaml`, `stats`, `collections`, `bytes`, `ip`, `archive-tar`) plus the
/// `kv-hashmap` package plugin.
pub fn populate_libraries(reg: &mut handl_kit_data::LibraryRegistry) {
    use handl_kit_data::LibraryPlugin;
    for name in [
        "hello",
        "utils",
        "cmp",
        "json",
        "toml",
        "yaml",
        "stats",
        "collections",
        "bytes",
        "ip",
        "archive-tar",
        "proc",
    ] {
        reg.register(name, move || {
            stdlib::library(name).unwrap_or_else(|| panic!("unknown stdlib package '{name}'"))
        });
    }
    // `kv-hashmap` — P-013/T-151 spec-driven: `lib-kv-hashmap/atoms.yaml`
    // (`module: kv`, 6 atoms) + `kv_impls()` (kernel `handl_ops::Op`s
    // adapted by `registry_from_atoms_kernel`) replace the
    // `KV_HASHMAP_OPS`/`kv_hashmap_op_registry`/`cap`-`op`-`handle`
    // decl triple. Atom names are the `kv.*` capability ids, same
    // surface the stripped `cap` decls declared.
    reg.register("kv-hashmap", || {
        use handl_platform::CapabilityId;
        use handl_plugin_lib_handl::HandlLibraryPlugin;
        let decls =
            stdlib::atoms::module_atoms(handl_plugin_lib_kv_hashmap::KV_HASHMAP_ATOMS, "kv")
                .expect("kv-hashmap atoms spec");
        let registry = stdlib::atoms::registry_from_atoms_kernel(
            &decls,
            &handl_plugin_lib_kv_hashmap::kv_impls(),
        )
        .expect("kv-hashmap atoms registry");
        let op_names: Vec<&str> = decls.iter().map(|a| a.name.as_str()).collect();
        let caps: Vec<CapabilityId> = decls
            .iter()
            .map(|a| CapabilityId::from(a.name.clone()))
            .collect();
        Box::new(
            HandlLibraryPlugin::new(
                "kv",
                "lib.kv-hashmap",
                "lib-kv-hashmap",
                handl_plugin_lib_kv_hashmap::KV_HASHMAP_HNL,
                &[],
            )
            .with_ops(&op_names, Arc::new(registry))
            .with_capabilities(caps),
        ) as LibraryPlugin
    });
}
