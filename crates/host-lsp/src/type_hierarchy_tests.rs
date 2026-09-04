//! Tests for `crate::type_hierarchy`.
//!
//! The cross-file tests (`supertypes_*`, `subtypes_*`) build a temporary
//! `arrows/` tree with a sibling `contracts/` directory on disk so that
//! `find_arrows_root` can locate the root and the type-hierarchy logic can
//! resolve predicate axiom locations in contract files.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::type_hierarchy::{prepare_type_hierarchy, subtypes, supertypes};

// ---------------------------------------------------------------------------
// Temp-directory helper
// ---------------------------------------------------------------------------

/// Monotonic counter so each test gets a unique temp directory, even when
/// tests run in parallel (where `SystemTime::now()` can return the same
/// nanosecond on separate threads).
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a unique temporary directory for a test, returning its path.
fn make_temp_dir(prefix: &str) -> PathBuf {
    let n = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!(
        "handl-lsp-type-hierarchy-{}-{}-{}",
        prefix,
        std::process::id(),
        n,
    ));
    fs::create_dir_all(&base).expect("create temp dir");
    base
}

/// Write a contract file under `contracts_dir/name.hnl` containing an
/// `axiom <predicate>` declaration, returning the file's path.
fn write_contract(contracts_dir: &PathBuf, file_name: &str, predicate: &str) -> PathBuf {
    fs::create_dir_all(contracts_dir).expect("create contracts dir");
    let path = contracts_dir.join(format!("{}.hnl", file_name));
    let contents = format!(
        "-- contracts\naxiom {} : List a -> Bool\n  guarantee: True\n",
        predicate
    );
    fs::write(&path, contents).expect("write contract file");
    path
}

// ---------------------------------------------------------------------------
// prepare_type_hierarchy
// ---------------------------------------------------------------------------

#[test]
fn prepare_type_hierarchy_for_predicate_returns_item() {
    // The cursor sits on the `Sorted` predicate name.
    let source = "Sorted";
    let offset = 0;

    let dir = make_temp_dir("prepare-pred");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");
    let current_file = arrows_dir.join("sort.hnl");
    fs::write(&current_file, source).expect("write current file");
    let current_str = current_file.to_string_lossy().to_string();

    let items = prepare_type_hierarchy(source, offset, Some(&current_str));
    assert_eq!(items.len(), 1, "expected exactly one type hierarchy item");

    let item = &items[0];
    assert_eq!(item["name"].as_str().unwrap(), "Sorted");
    assert_eq!(
        item["kind"].as_u64().unwrap(),
        5,
        "kind should be TypeParameter (5)"
    );
    assert_eq!(item["detail"].as_str().unwrap(), "predicate");
    // selectionRange should cover the `Sorted` identifier (offsets 0..6).
    assert_eq!(item["selectionRange"]["start"]["line"].as_u64().unwrap(), 0);
    assert_eq!(
        item["selectionRange"]["start"]["character"].as_u64().unwrap(),
        0
    );
    assert_eq!(
        item["selectionRange"]["end"]["character"].as_u64().unwrap(),
        6
    );
    // uri should point at the current document.
    assert!(
        item["uri"].as_str().unwrap().ends_with("sort.hnl"),
        "uri should point at sort.hnl: {}",
        item["uri"]
    );
}

#[test]
fn prepare_type_hierarchy_for_non_identifier_returns_empty() {
    // `42` is a number literal, not an identifier.
    let source = "42";
    let items = prepare_type_hierarchy(source, 0, None);
    assert!(
        items.is_empty(),
        "expected no items for a non-identifier, got {:?}",
        items
    );
}

#[test]
fn prepare_type_hierarchy_empty_source_returns_empty() {
    assert!(prepare_type_hierarchy("", 0, None).is_empty());
}

// ---------------------------------------------------------------------------
// supertypes
// ---------------------------------------------------------------------------

#[test]
fn supertypes_for_sorted_returns_is_perm() {
    let dir = make_temp_dir("sup-sorted");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib");
    let contracts_dir = dir.join("handl-packages").join("kit-stdlib").join("contracts");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");

    // Contract file defining `axiom IsPerm` (the supertype).
    write_contract(&contracts_dir, "list", "IsPerm");

    // The current file lives under arrows/ so find_arrows_root can locate the
    // root and the sibling contracts/ directory.
    let current_file = arrows_dir.join("sort.hnl");
    fs::write(&current_file, "Sorted").expect("write current file");
    let current_str = current_file.to_string_lossy().to_string();

    let supers = supertypes("Sorted", 0, Some(&current_str));
    assert_eq!(
        supers.len(),
        1,
        "expected exactly one supertype (IsPerm), got {:?}",
        supers
    );

    let item = &supers[0];
    assert_eq!(item["name"].as_str().unwrap(), "IsPerm");
    assert_eq!(item["kind"].as_u64().unwrap(), 5);
    // uri should point at the contract file defining `axiom IsPerm`.
    assert!(
        item["uri"].as_str().unwrap().ends_with("list.hnl"),
        "supertype uri should point at list.hnl: {}",
        item["uri"]
    );
}

#[test]
fn supertypes_for_base_predicate_returns_empty() {
    let dir = make_temp_dir("sup-base");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib");
    let contracts_dir = dir.join("handl-packages").join("kit-stdlib").join("contracts");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");
    // `LengthEq` is a base predicate with no supertypes.
    write_contract(&contracts_dir, "list", "LengthEq");

    let current_file = arrows_dir.join("rev.hnl");
    fs::write(&current_file, "LengthEq").expect("write current file");
    let current_str = current_file.to_string_lossy().to_string();

    let supers = supertypes("LengthEq", 0, Some(&current_str));
    assert!(
        supers.is_empty(),
        "base predicate LengthEq should have no supertypes, got {:?}",
        supers
    );
}

// ---------------------------------------------------------------------------
// subtypes
// ---------------------------------------------------------------------------

#[test]
fn subtypes_for_is_perm_returns_sorted() {
    let dir = make_temp_dir("sub-isperm");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib");
    let contracts_dir = dir.join("handl-packages").join("kit-stdlib").join("contracts");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");

    // Contract file defining `axiom Sorted` (the subtype).
    write_contract(&contracts_dir, "list", "Sorted");

    let current_file = arrows_dir.join("perm.hnl");
    fs::write(&current_file, "IsPerm").expect("write current file");
    let current_str = current_file.to_string_lossy().to_string();

    let subs = subtypes("IsPerm", 0, Some(&current_str));
    assert!(
        !subs.is_empty(),
        "expected at least one subtype (Sorted), got {:?}",
        subs
    );

    // `Sorted` should appear among the subtypes.
    let has_sorted = subs
        .iter()
        .any(|item| item["name"].as_str() == Some("Sorted"));
    assert!(
        has_sorted,
        "expected `Sorted` among subtypes of IsPerm, got {:?}",
        subs
    );

    // The `Sorted` subtype item should point at the contract file.
    let sorted_item = subs
        .iter()
        .find(|item| item["name"].as_str() == Some("Sorted"))
        .expect("Sorted subtype item should exist");
    assert_eq!(sorted_item["kind"].as_u64().unwrap(), 5);
    assert!(
        sorted_item["uri"].as_str().unwrap().ends_with("list.hnl"),
        "Sorted subtype uri should point at list.hnl: {}",
        sorted_item["uri"]
    );
}

#[test]
fn subtypes_empty_source_returns_empty() {
    assert!(subtypes("", 0, None).is_empty());
}
