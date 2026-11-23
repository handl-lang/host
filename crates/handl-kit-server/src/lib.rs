//! `handl-kit-server` — the OS-adjacent / C-library-backed kit.
//!
//! Extracted from `handl-kit-canonical`: the modules whose `.hnl` surfaces
//! sit on system libraries — `crypto` (libsodium), `intl_c`/`unicode_c`
//! (ICU4C via `ffi`), `entropy` — plus the `config`/`server` kit
//! manifests' `LibraryRegistry` rows (`datetime`, `otlp-std`, …).

use std::sync::Arc;

use handl_kit_core::{ModuleCtor, ModuleEntry};
use handl_plugin_lib_stdlib as stdlib;

// ---------------------------------------------------------------------------
// registration
// ---------------------------------------------------------------------------

/// The kit's module table — all table-built `stdlib::module_with` rows
/// whose deps (`ffi`/`char`/`type`/`tag`/`error`/`math`) resolve through
/// the shared kit resolver.
pub const MODULES: &[(&'static str, ModuleCtor)] = &[
    ("crypto", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("crypto", Arc::new(*r))
                .expect("crypto")
                .with_eval_engine(e),
        )
    }),
    ("intl_c", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("intl_c", Arc::new(*r))
                .expect("intl_c")
                .with_eval_engine(e),
        )
    }),
    ("unicode_c", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("unicode_c", Arc::new(*r))
                .expect("unicode_c")
                .with_eval_engine(e),
        )
    }),
    ("entropy", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("entropy", Arc::new(*r))
                .expect("entropy")
                .with_eval_engine(e),
        )
    }),
    // `otlp` — the conn-as-value `.hnl` contract (`otlp.configure(url) ->
    // conn`); its `otlp_c.*` calls dispatch through the resolver to the
    // `otlp_c` native-ops row below.
    ("otlp", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("otlp", Arc::new(*r))
                .expect("otlp")
                .with_eval_engine(e),
        )
    }),
    // `otlp_c` — the Rust FFI backend (`lib-otlp-c` `otlp_c.*` ops over
    // `lib-otlp-std`'s HTTP exporter registry). `funcs: &[]` — the record
    // is cap/op decls only; `ops` exposes each op under its local name.
    // `icu_*` — codegen-generated ICU4C adapter modules that `intl_c`/`unicode_c`
    // reach via `from "../generated/icu*/icu*-linux-x86_64/<mod>_c"` path imports
    // (basename-normalized in `packages.rs`). Registered so `call_host`
    // `ucol.ucol_open_78`-style wires resolve in the outer registry.
    ("ucol", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("ucol", Arc::new(*r))
                .expect("ucol")
                .with_eval_engine(e),
        )
    }),
    ("unum", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("unum", Arc::new(*r))
                .expect("unum")
                .with_eval_engine(e),
        )
    }),
    ("upluralrules", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("upluralrules", Arc::new(*r))
                .expect("upluralrules")
                .with_eval_engine(e),
        )
    }),
    ("ureldatefmt", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("ureldatefmt", Arc::new(*r))
                .expect("ureldatefmt")
                .with_eval_engine(e),
        )
    }),
    ("udat", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("udat", Arc::new(*r))
                .expect("udat")
                .with_eval_engine(e),
        )
    }),
    ("ufieldpositer", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("ufieldpositer", Arc::new(*r))
                .expect("ufieldpositer")
                .with_eval_engine(e),
        )
    }),
    ("unumberformatter", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("unumberformatter", Arc::new(*r))
                .expect("unumberformatter")
                .with_eval_engine(e),
        )
    }),
    ("uformattednumber", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("uformattednumber", Arc::new(*r))
                .expect("uformattednumber")
                .with_eval_engine(e),
        )
    }),
    ("ubrk", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("ubrk", Arc::new(*r))
                .expect("ubrk")
                .with_eval_engine(e),
        )
    }),
    ("uloc", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("uloc", Arc::new(*r))
                .expect("uloc")
                .with_eval_engine(e),
        )
    }),
    ("uldnames", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("uldnames", Arc::new(*r))
                .expect("uldnames")
                .with_eval_engine(e),
        )
    }),
    ("ures", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("ures", Arc::new(*r))
                .expect("ures")
                .with_eval_engine(e),
        )
    }),
    ("unorm2", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("unorm2", Arc::new(*r))
                .expect("unorm2")
                .with_eval_engine(e),
        )
    }),
    ("ustring", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("ustring", Arc::new(*r))
                .expect("ustring")
                .with_eval_engine(e),
        )
    }),
    ("uchar", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("uchar", Arc::new(*r))
                .expect("uchar")
                .with_eval_engine(e),
        )
    }),
    ("otlp_c", |r, e| {
        ModuleEntry::Handl(
            stdlib::module_with("otlp_c", Arc::new(*r))
                .expect("otlp_c")
                .with_eval_engine(e),
        )
    }),
];

/// Register the server kit's modules into the shared kit table.
/// `Once`-safe to call repeatedly.
pub fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| handl_kit_core::register_modules(MODULES));
}

// ---------------------------------------------------------------------------
// LibraryRegistry rows — the `config`/`server` manifests' `modules:` names
// resolve through this table (ADR-0128 §0.4).
// ---------------------------------------------------------------------------

/// Populate `reg` with this kit's library-module constructors.
pub fn populate_libraries(reg: &mut handl_kit_data::LibraryRegistry) {
    use handl_kit_data::LibraryPlugin;
    for name in ["crypto", "datetime", "intl_c", "unicode_c"] {
        reg.register(name, move || {
            stdlib::library(name).unwrap_or_else(|| panic!("unknown stdlib package '{name}'"))
        });
    }
    // `otlp-std` — the `handl-ops`-migrated OTLP/HTTP exporter: the lib
    // repo exposes a kernel `handl_ops::OpRegistry`; the host
    // `HandlLibraryPlugin` wrapper (formerly
    // `handl_plugin_lib_otlp_std::LibraryPlugin`) lives here now.
    reg.register("otlp-std", || {
        use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
        let mut registry = OpRegistry::new();
        registry.import_kernel(&handl_plugin_lib_otlp_std::otlp_std_op_registry());
        Box::new(
            HandlLibraryPlugin::new_with_caps(
                "otlp_c",
                "lib.otlp-std",
                "lib-otlp-std",
                handl_plugin_lib_otlp_std::OTLP_STD_HNL,
                &[],
            )
            .with_ops(handl_plugin_lib_otlp_std::OTLP_OPS, Arc::new(registry)),
        ) as LibraryPlugin
    });
}
