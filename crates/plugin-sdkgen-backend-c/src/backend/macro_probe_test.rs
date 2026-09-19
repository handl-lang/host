use super::clang_args::clang_args;
use super::macro_probe::eval_macros;
use clang_sys::*;
use std::path::Path;

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple.h")
}

#[test]
fn evaluates_object_like_macros_only() {
    clang_sys::load().expect("libclang load");
    let header = fixture();
    let index = unsafe { clang_createIndex(0, 0) };
    let (_raw, args) = clang_args(&header, &target_lexicon::Triple::host().to_string(), &[]);
    let names = vec![
        "TEST_AF_INET".to_string(),   // plain integer macro
        "TEST_MAX".to_string(),       // function-like: cannot fold
        "SIMPLE_H".to_string(),       // empty include guard
        "DOES_NOT_EXIST".to_string(), // undeclared identifier
    ];
    let out = eval_macros(index, &args, &names, &header);
    unsafe { clang_disposeIndex(index) };

    let af = out
        .iter()
        .find(|c| c.name == "TEST_AF_INET")
        .expect("AF_INET");
    assert_eq!(af.value, 2);
    for gone in ["TEST_MAX", "SIMPLE_H", "DOES_NOT_EXIST"] {
        assert!(!out.iter().any(|c| c.name == gone), "{gone} leaked");
    }
}

#[test]
fn empty_name_list_short_circuits() {
    // No TU should be parsed; passing a null index must not be touched.
    let header = fixture();
    let out = eval_macros(std::ptr::null_mut(), &[], &[], &header);
    assert!(out.is_empty());
}
