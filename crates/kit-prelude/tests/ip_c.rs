//! `ip.*` smoke tests — the `ip` module's op registry is the native
//! `lib-ip-c` backend (spec-driven via `lib-ip-c/atoms.yaml` +
//! `ip_impls()`, attached in `handl-kit-stdlib`'s `build_ip_module`).
//!
//! IPv6 inputs are the tell: the pure-`.hnl` `lib-ip` surface only
//! parses IPv4, so `ip.parse("::1")` succeeding proves the native ops
//! are the ones dispatching.

use handl_kit_prelude::Kit;
use serde_json::json;

fn eval(src: &str) -> serde_json::Value {
    let wrapped = format!("do {{ {src} }}");
    Kit::new()
        .expect("canonical kit must assemble")
        .eval(&wrapped)
        .unwrap_or_else(|e| panic!("`{src}` must eval: {e}"))
}

#[test]
fn ip_parse_ipv6_native() {
    let v = eval(r#"ip.parse("::1")"#);
    assert_eq!(v["version"], json!(6));
    assert_eq!(v["address"], json!("::1"));
    assert_eq!(v["is_loopback"], json!(true));
}

#[test]
fn ip_parse_ipv4_native() {
    let v = eval(r#"ip.parse("192.168.1.1")"#);
    assert_eq!(v["version"], json!(4));
    assert_eq!(v["address"], json!("192.168.1.1"));
    assert_eq!(v["is_private"], json!(true));
}

#[test]
fn ip_cidr_and_socket_native() {
    assert_eq!(eval(r#"ip.cidr.parse("10.0.0.0/8").prefix"#), json!(8.0));
    assert_eq!(
        eval(r#"ip.cidr.network("192.168.0.0/24")"#),
        json!("192.168.0.0")
    );
    assert_eq!(
        eval(r#"ip.socket.parse("127.0.0.1:8080").port"#),
        json!(8080.0)
    );
    assert_eq!(
        eval(r#"ip.format(ip.parse("2001:db8::1"))"#),
        json!("2001:db8::1")
    );
}
