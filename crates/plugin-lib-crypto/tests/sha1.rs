//! Integration tests for the pure-handl `crypto.sha1` implementation.
//!
//! `crypto.sha1` is a Handl arrow in `handl-packages/crypto/crypto.hnl`
//! (over the `math`/`bytes`/`typeof` atoms) — not a Rust handl. These tests
//! exercise it through `CryptoModule` exactly as the eval engines would.
//! Vectors are FIPS 180-1 / RFC 3174 plus the RFC 6455 WebSocket handshake.

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_crypto::CryptoModule;
use serde_json::json;

fn sha1(input: serde_json::Value) -> String {
    CryptoModule
        .function("sha1")
        .expect("crypto.sha1 must be exposed")
        .call(&[input], &mut EvalContext::new())
        .expect("crypto.sha1 call failed")
        .as_str()
        .expect("crypto.sha1 must return a hex string")
        .to_string()
}

#[test]
fn sha256_and_md5_still_resolve() {
    // Regression: the Rust handls must keep working alongside the new
    // pure-handl `sha1` record field.
    let sha256 = CryptoModule
        .function("sha256")
        .expect("crypto.sha256")
        .call(&[json!("abc")], &mut EvalContext::new())
        .expect("crypto.sha256 call failed");
    assert_eq!(
        sha256.as_str().unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let md5 = CryptoModule
        .function("md5")
        .expect("crypto.md5")
        .call(&[json!("abc")], &mut EvalContext::new())
        .expect("crypto.md5 call failed");
    assert_eq!(md5.as_str().unwrap(), "900150983cd24fb0d6963f7d28e17f72");
}

#[test]
fn sha1_empty_string() {
    assert_eq!(sha1(json!("")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn sha1_abc() {
    assert_eq!(
        sha1(json!("abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn sha1_quick_brown_fox() {
    assert_eq!(
        sha1(json!("The quick brown fox jumps over the lazy dog")),
        "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
    );
}

#[test]
fn sha1_byte_list() {
    assert_eq!(
        sha1(json!([97, 98, 99])),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn sha1_bytes_buffer() {
    // `Bytes` arrives across the bridge as the `{ "__bytes__": [...] }`
    // tagged-scalar object.
    assert_eq!(
        sha1(json!({ "__bytes__": [97, 98, 99] })),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn sha1_pad_boundary_55() {
    // 55-byte message: the 0x80 pad byte lands at index 56 — one block.
    let msg = "a".repeat(55);
    // python: hashlib.sha1(b"a" * 55).hexdigest()
    assert_eq!(sha1(json!(msg)), "c1c8bbdc22796e28c0e15163d20899b65621d65a");
}

#[test]
fn sha1_pad_boundary_56() {
    // 56-byte message: padding spills into a second block.
    let msg = "a".repeat(56);
    // python: hashlib.sha1(b"a" * 56).hexdigest()
    assert_eq!(sha1(json!(msg)), "c2db330f6083854c99d4b5bfb6e8f29f201be699");
}

#[test]
fn sha1_multi_block() {
    // 64-byte message → two 512-bit blocks after padding.
    let msg = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl";
    assert_eq!(sha1(json!(msg)), "93249d4c2f8903ebf41ac358473148ae6ddd7042");
}

#[test]
fn sha1_websocket_accept() {
    // RFC 6455 §1.3: Sec-WebSocket-Accept =
    //   base64(sha1(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))
    // For key "dGhlIHNhbXBsZSBub25jZQ==" the base64 accept value is
    // "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="; asserted here as the equivalent hex
    // digest (the raw digest bytes are not UTF-8, so base64.encode —
    // string-only — cannot wrap them).
    assert_eq!(
        sha1(json!(
            "dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
        )),
        "b37a4f2cc0624f1690f64606cf385945b2bec4ea"
    );
}
