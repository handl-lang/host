//! Concrete library-plugin registry population for the built-in kits.
//!
//! This module depends on the concrete `plugin-lib-*` crates so it can
//! register their zero-argument constructors in a [`handl_kit_data::LibraryRegistry`].

use handl_kit_data::{LibraryPlugin, LibraryRegistry};

/// Populate `reg` with a constructor for every library plugin used by the
/// built-in kits (`hello`, `stdlib`, `cli`, `config`, `server`) plus the
/// shared `module-runtime` plugin.
pub fn populate(reg: &mut LibraryRegistry) {
    reg.register("module-runtime", || {
        Box::new(handl_plugin_module::ModuleRuntime::default()) as LibraryPlugin
    });
    reg.register("hello", || {
        Box::new(handl_plugin_lib_hello::HelloLibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("os-std", || {
        Box::new(handl_plugin_lib_os_std::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("utils", || {
        Box::new(handl_plugin_lib_utils::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("cmp", || {
        Box::new(handl_plugin_lib_cmp::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("collections", || {
        Box::new(handl_plugin_lib_collections::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("json", || {
        Box::new(handl_plugin_lib_json::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("toml", || {
        Box::new(handl_plugin_lib_toml::LibraryPlugin::new()) as LibraryPlugin
    });
    reg.register("yaml", || {
        Box::new(handl_plugin_lib_yaml::LibraryPlugin::new()) as LibraryPlugin
    });
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
    reg.register("stats", || {
        Box::new(handl_plugin_lib_stats::LibraryPlugin::new()) as LibraryPlugin
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
        Box::new(handl_plugin_lib_archive_tar::LibraryPlugin::new()) as LibraryPlugin
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
