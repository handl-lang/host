use crate::collect::{CursorData, collect_cursors, collect_enum};
use crate::cx::cursor_name;
use crate::testutil::{Snippet, parse_snippet};
use clang_sys::*;
use glob::Pattern;
use handl_plugin_sdkgen::SdkgenModule;
use std::ffi::c_void;

const SRC: &str = "\
typedef unsigned long size_t;
#define OBJ_CONST 42
#define FN_LIKE(x) ((x) + 1)
struct defined { int x; };
struct forward;
typedef int myint;
enum Color { RED, GREEN };
enum { ANON_A = 10 };
int pub_fn(int a);
static int internal_fn(int a);
";

fn collect(src: &str, only: Option<&str>) -> (Snippet, CursorData) {
    collect_with(src, only, &[])
}

fn collect_with(src: &str, only: Option<&str>, consts: &[&str]) -> (Snippet, CursorData) {
    let sn = parse_snippet(src);
    let mut data = CursorData {
        only: only
            .map(|p| vec![Pattern::new(p).unwrap()])
            .unwrap_or_default(),
        consts: consts.iter().map(|p| Pattern::new(p).unwrap()).collect(),
        ..Default::default()
    };
    let root = unsafe { clang_getTranslationUnitCursor(sn.tu()) };
    unsafe { clang_visitChildren(root, collect_cursors, &mut data as *mut _ as *mut c_void) };
    (sn, data)
}

fn names(cursors: &[CXCursor]) -> Vec<String> {
    cursors.iter().map(|&c| cursor_name(c)).collect()
}

#[test]
fn collects_each_declaration_kind() {
    let (_sn, d) = collect(SRC, None);
    assert_eq!(names(&d.functions), ["pub_fn"]);
    assert_eq!(names(&d.records), ["defined"]);
    assert_eq!(names(&d.typedefs), ["size_t", "myint"]);
    assert_eq!(d.enums.len(), 2);
    assert_eq!(names(&d.macros), ["OBJ_CONST"]);
}

#[test]
fn skips_internal_linkage_and_forward_decls() {
    let (_sn, d) = collect(SRC, None);
    // `static` functions have no external linkage.
    assert!(!names(&d.functions).contains(&"internal_fn".to_string()));
    // `struct forward;` is a declaration, not a definition.
    assert!(!names(&d.records).contains(&"forward".to_string()));
}

#[test]
fn only_filter_gates_functions_records_typedefs_macros() {
    let (_sn, d) = collect(SRC, Some("pub_*"));
    assert_eq!(names(&d.functions), ["pub_fn"]);
    assert!(d.records.is_empty());
    assert!(d.typedefs.is_empty());
    assert!(d.macros.is_empty());
    // Enums are exempt at decl level (constants carry the names).
    assert_eq!(d.enums.len(), 2);
}

#[test]
fn collect_enum_filters_constants_by_only() {
    let (_sn, d) = collect(SRC, Some("ANON_*"));
    let mut module = SdkgenModule::default();
    for &e in &d.enums {
        collect_enum(e, &mut module, &d.only, &d.consts);
    }
    assert_eq!(module.enum_constants.len(), 1);
    assert_eq!(module.enum_constants[0].name, "ANON_A");
    assert_eq!(module.enum_constants[0].value, 10);
}

#[test]
fn collect_enum_no_filter_takes_all() {
    let (_sn, d) = collect(SRC, None);
    let mut module = SdkgenModule::default();
    for &e in &d.enums {
        collect_enum(e, &mut module, &d.only, &d.consts);
    }
    let got: Vec<&str> = module
        .enum_constants
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(got, ["RED", "GREEN", "ANON_A"]);
}

#[test]
fn consts_glob_gates_enums_and_macros_independently() {
    // `constants` selects enum constants + macros without touching the
    // function/record/typedef surface.
    let (_sn, d) = collect_with(SRC, None, &["ANON_*", "OBJ_*"]);
    assert_eq!(names(&d.functions), ["pub_fn"]);
    assert_eq!(names(&d.records), ["defined"]);
    assert_eq!(names(&d.macros), ["OBJ_CONST"]); // FN_LIKE is function-like

    let mut module = SdkgenModule::default();
    for &e in &d.enums {
        collect_enum(e, &mut module, &d.only, &d.consts);
    }
    let got: Vec<&str> = module
        .enum_constants
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(got, ["ANON_A"]);
}

#[test]
fn consts_glob_still_applies_under_only() {
    let (_sn, d) = collect_with(SRC, Some("pub_*"), &["RED"]);
    assert_eq!(names(&d.functions), ["pub_fn"]);
    let mut module = SdkgenModule::default();
    for &e in &d.enums {
        collect_enum(e, &mut module, &d.only, &d.consts);
    }
    assert_eq!(module.enum_constants.len(), 1);
    assert_eq!(module.enum_constants[0].name, "RED");
}
