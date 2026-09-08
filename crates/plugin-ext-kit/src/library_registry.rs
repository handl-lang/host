//! Concrete library-plugin registry population for the built-in kits.
//!
//! Handl-backed libraries (`hello`, `utils`, `cmp`, `json`, `toml`,
//! `yaml`, `stats`, `collections`) are built from the package table in
//! [`handl_plugin_lib_stdlib::packages`]; libraries with their own
//! runtime/native backends keep their crate-provided plugins.

use handl_kit_data::{LibraryPlugin, LibraryRegistry};

/// Populate `reg` with a constructor for every library plugin used by the
/// built-in kits (`hello`, `stdlib`, `cli`, `config`, `server`) plus the
/// shared `module-runtime` plugin.
pub fn populate(reg: &mut LibraryRegistry) {
    reg.register("module-runtime", || {
        Box::new(handl_plugin_module::ModuleRuntime::default()) as LibraryPlugin
    });
    reg.register("os-std", || {
        Box::new(handl_plugin_lib_os_std::LibraryPlugin::new()) as LibraryPlugin
    });
    // Table-driven libraries (former plugin-lib-{hello,utils,cmp,json,
    // toml,yaml,stats,collections}).
    for name in [
        "hello",
        "utils",
        "cmp",
        "json",
        "toml",
        "yaml",
        "stats",
        "collections",
    ] {
        reg.register(name, move || {
            handl_plugin_lib_stdlib::library(name)
                .unwrap_or_else(|| panic!("unknown stdlib package '{name}'"))
        });
    }
    reg.register("crypto", || {
        Box::new(handl_plugin_lib_crypto::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("datetime", || {
        Box::new(handl_plugin_lib_datetime::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("intl", || {
        Box::new(handl_plugin_lib_intl::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("kv-hashmap", || {
        Box::new(handl_plugin_lib_kv_hashmap::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("unicode", || {
        Box::new(handl_plugin_lib_unicode::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("ws-rfc-memory", || {
        Box::new(handl_plugin_lib_ws_rfc_memory::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("ws-whatwg-native", || {
        Box::new(handl_plugin_lib_ws_whatwg_native::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("ip", || {
        Box::new(handl_plugin_lib_ip::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("compress-gzip", || {
        Box::new(handl_plugin_lib_compress_gzip::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("archive-tar", || {
        handl_plugin_lib_stdlib::library("archive-tar")
            .unwrap_or_else(|| panic!("unknown stdlib package 'archive-tar'"))
    });
    reg.register("rand-native", || {
        Box::new(handl_plugin_lib_rand_native::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("http-client-std", || {
        Box::new(handl_plugin_lib_http_client_std::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("otlp-std", || {
        Box::new(handl_plugin_lib_otlp_std::LibraryPlugin::new()) as LibraryPlugin
    });
}
