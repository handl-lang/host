//! Integration tests for the `ip` package row.
//!
//! `ip` is a pure `.hnl` table module now (`lib-ip`'s flattened single
//! source, `IP_FLAT_HNL`) — the old `IpModule` Rust aggregate is gone
//! (BACKLOG: "`ip` pure `.hnl` (`ip.cidr.*`/`ip.socket.*` nested records);
//! `IpModule` gone"). These tests pin both halves: the `library("ip")`
//! row and the dotted sub-module dispatch (`ip` / `cidr` / `socket`)
//! through `module_with("ip", …)`.
//!
//! The flat `.hnl` calls `char.*`/`str.*`/`math.*`/`typeof.*`. `char` is a
//! contract-only floor module (no table row — a table `char` would shadow
//! the host's native module), so the resolver must carry it as a fallback,
//! exactly like production's `module_with(name, StdlibResolver)` path.

use std::sync::Arc;

use handl_platform::{EvalContext, Module, Plugin};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{HandlModule, ModuleResolver};
use handl_plugin_lib_stdlib::{PackageResolver, library, module_with};
use serde_json::json;

/// Install the canonical default language once per test binary (see
/// `json.rs` — the `HandlModule` bridge runtimes resolve it lazily).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

/// Floor-module resolver: native `char` plus the package table — the same
/// resolver shape production passes to `module_with`.
fn floor_resolver() -> Arc<dyn ModuleResolver> {
    struct CharFloor;
    impl ModuleResolver for CharFloor {
        fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
            (name == "char").then(|| Arc::new(CharModule) as Arc<dyn Module + Send + Sync>)
        }
    }
    Arc::new(PackageResolver {
        fallback: Some(Arc::new(CharFloor)),
    })
}

fn table_module() -> HandlModule {
    ensure_lang();
    module_with("ip", floor_resolver()).expect("ip module in the package table")
}

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    module
        .function(name)
        .unwrap_or_else(|| panic!("ip.{name} bound"))
        .call(args, &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("ip.{name} evaluates: {e}"))
}

#[test]
fn ip_package_row_exists() {
    let plugin = library("ip").expect("ip package should exist");
    assert_eq!(plugin.id(), "lib.ip");
    assert_eq!(plugin.name(), "lib-ip");

    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 14, "ip declares 14 capabilities");
    assert!(caps.iter().any(|c| c.as_str() == "ip.parse"));
    assert!(caps.iter().any(|c| c.as_str() == "ip.cidr.parse"));
    assert!(caps.iter().any(|c| c.as_str() == "ip.socket.format"));
}

#[test]
fn ip_module_dispatches_ip_and_cidr() {
    let module = table_module();
    assert_eq!(module.name(), "ip");

    let ip = call(&module, "parse", &[json!("192.168.1.1")]);
    assert_eq!(ip["address"], "192.168.1.1");
    assert_eq!(ip["version"], 4);
    assert_eq!(ip["is_private"], true);

    let cidr = call(&module, "cidr.parse", &[json!("192.168.0.0/24")]);
    assert_eq!(cidr["address"], "192.168.0.0");
    assert_eq!(cidr["prefix"].as_f64(), Some(24.0));
}

#[test]
fn ip_cidr_and_socket_ops() {
    let module = table_module();

    let network = call(&module, "cidr.network", &[json!("10.0.0.0/8")]);
    assert_eq!(network, json!("10.0.0.0"));

    let contains = call(
        &module,
        "cidr.contains",
        &[json!("10.0.0.0/8"), json!("10.1.2.3")],
    );
    assert_eq!(contains, json!(true));

    let socket = call(&module, "socket.format", &[json!("[::1]:443")]);
    assert_eq!(socket, json!("[::1]:443"));
}
