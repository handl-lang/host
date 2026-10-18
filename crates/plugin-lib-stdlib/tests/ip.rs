//! Integration tests for the `ip` package row.
//!
//! `ip` was extracted to its own repo (`lib-ip`, ADR-0128); the stdlib
//! re-exports the crate as `ops::ip` and contributes the `IpModule`
//! aggregate. These tests lock both halves: the `library("ip")` row and
//! the dotted sub-module dispatch (`ip` / `cidr` / `socket`).

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_stdlib::ops::ip::IpModule;
use serde_json::json;

#[test]
fn ip_package_row_exists() {
    let plugin = handl_plugin_lib_stdlib::library("ip").expect("ip package should exist");
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
    let module = IpModule;
    assert_eq!(module.name(), "ip");
    let mut ctx = EvalContext::new();

    let parse = module.function("parse").expect("ip.parse");
    let ip = parse
        .call(&[json!("192.168.1.1")], &mut ctx)
        .expect("ip.parse");
    assert_eq!(ip["address"], "192.168.1.1");
    assert_eq!(ip["version"], 4);
    assert_eq!(ip["is_private"], true);

    let cidr_parse = module.function("cidr.parse").expect("ip.cidr.parse");
    let cidr = cidr_parse
        .call(&[json!("192.168.0.0/24")], &mut ctx)
        .expect("ip.cidr.parse");
    assert_eq!(cidr["address"], "192.168.0.0");
    assert_eq!(cidr["prefix"].as_f64(), Some(24.0));
}

#[test]
fn ip_cidr_and_socket_ops() {
    let module = IpModule;
    let mut ctx = EvalContext::new();

    let network = module.function("cidr.network").expect("ip.cidr.network");
    assert_eq!(
        network
            .call(&[json!("10.0.0.0/8")], &mut ctx)
            .expect("cidr.network"),
        json!("10.0.0.0")
    );

    let contains = module.function("cidr.contains").expect("ip.cidr.contains");
    assert_eq!(
        contains
            .call(&[json!("10.0.0.0/8"), json!("10.1.2.3")], &mut ctx)
            .expect("cidr.contains"),
        json!(true)
    );

    let socket = module.function("socket.format").expect("ip.socket.format");
    assert_eq!(
        socket
            .call(&[json!("[::1]:443")], &mut ctx)
            .expect("socket.format"),
        json!("[::1]:443")
    );
}
