use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::refinement::RefinementEngine;
use crate::type_definition::type_definition;

/// Monotonic counter so each test gets a unique temp directory, even when
/// tests run in parallel (where `SystemTime::now()` can return the same
/// nanosecond on separate threads).
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a unique temporary directory for a test, returning its path.
fn make_temp_dir(prefix: &str) -> PathBuf {
    let n = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!(
        "handl-lsp-type-def-{}-{}-{}",
        prefix,
        std::process::id(),
        n,
    ));
    fs::create_dir_all(&base).expect("create temp dir");
    base
}

/// A simple arrow file source: `id >>> id`.
///
/// This parses as an arrow file and infers to `a ~> a` with guarantee `True`.
const ID_COMP: &str = "id >>> id";

#[test]
fn type_definition_for_arrow_name_returns_location() {
    let dir = make_temp_dir("arrow");
    let arrows_dir = dir.join("handl-packages");
    let contracts_dir = dir.join("handl-packages").join("contracts");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");
    fs::create_dir_all(&contracts_dir).expect("create contracts dir");

    // Write a contract file containing `axiom id`.
    let list_path = contracts_dir.join("list.hnl");
    fs::write(&list_path, "-- contracts\naxiom id : a -> a\n  guarantee: True\n")
        .expect("write list.hnl");

    // The current file lives under arrows/ so find_arrows_root can locate
    // the root and the sibling contracts/ directory.
    let current_file = arrows_dir.join("test.hnl");
    fs::write(&current_file, ID_COMP).expect("write test.hnl");

    let current_str = current_file.to_string_lossy().to_string();
    let engine = RefinementEngine::new();

    // Offset pointing at the first `id` identifier (offset 0).
    let loc = type_definition(ID_COMP, 0, Some(&current_str), &engine)
        .expect("should find type definition for arrow name `id`");

    let uri = loc["uri"].as_str().unwrap();
    assert!(
        uri.ends_with("list.hnl"),
        "type definition uri should point at list.hnl: {}",
        uri
    );
    // The range should point at the `axiom id` line.
    let start_line = loc["range"]["start"]["line"].as_u64().unwrap();
    let start_char = loc["range"]["start"]["character"].as_u64().unwrap();
    assert_eq!(start_line, 1, "should point at the axiom line");
    assert_eq!(start_char, 0, "should start at beginning of `axiom id`");
}

#[test]
fn type_definition_for_non_identifier_returns_none() {
    let engine = RefinementEngine::new();
    // `42` is a number literal, not an identifier.
    let source = "42";
    assert!(type_definition(source, 0, None, &engine).is_none());
}

#[test]
fn type_definition_for_type_predicate_finds_axiom() {
    let dir = make_temp_dir("pred");
    let arrows_dir = dir.join("handl-packages");
    let contracts_dir = dir.join("handl-packages").join("contracts");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");
    fs::create_dir_all(&contracts_dir).expect("create contracts dir");

    // Write a contract file containing `axiom Sorted`.
    let list_path = contracts_dir.join("list.hnl");
    fs::write(&list_path, "-- list contracts\naxiom Sorted : List a -> Bool\n  guarantee: True\n")
        .expect("write list.hnl");

    // The current file lives under arrows/ so the contracts directory can
    // be located. The source is just the predicate name `Sorted`.
    let current_file = arrows_dir.join("sort.hnl");
    fs::write(&current_file, "Sorted").expect("write sort.hnl");

    let current_str = current_file.to_string_lossy().to_string();
    let engine = RefinementEngine::new();

    // Offset pointing at `Sorted` (offset 0).
    let loc = type_definition("Sorted", 0, Some(&current_str), &engine)
        .expect("should find axiom for type predicate `Sorted`");

    let uri = loc["uri"].as_str().unwrap();
    assert!(
        uri.ends_with("list.hnl"),
        "type predicate uri should point at list.hnl: {}",
        uri
    );
    // The range should point at the `axiom Sorted` line.
    let start_line = loc["range"]["start"]["line"].as_u64().unwrap();
    assert_eq!(start_line, 1, "should point at the axiom line");
}

#[test]
fn type_definition_empty_source_returns_none() {
    let engine = RefinementEngine::new();
    assert!(type_definition("", 0, None, &engine).is_none());
}

#[test]
fn type_definition_no_match_returns_none() {
    let engine = RefinementEngine::new();
    // `foobar` is an identifier but not a type predicate, not a known
    // arrow, and no file_path is provided so no contracts can be searched.
    let source = "foobar";
    assert!(type_definition(source, 0, None, &engine).is_none());
}
