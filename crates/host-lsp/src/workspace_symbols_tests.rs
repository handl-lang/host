//! Tests for `workspace_symbols`.
//!
//! These build throwaway directory trees under `std::env::temp_dir()` that
//! mimic the `kit-stdlib/data/arrows` and `kit-stdlib/data/contracts` layout,
//! then exercise `workspace_symbols` against them.

use std::fs;
use std::env::temp_dir;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::workspace_symbols::workspace_symbols;

/// Monotonic counter so each test gets a unique temp directory.
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a unique temporary directory for a test and return its path.
fn unique_temp_dir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = temp_dir().join(format!("handl-ws-symbols-{}-{}", label, n));
    // Clean up any stale directory from a previous run, then recreate.
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write `contents` to `dir/rel` creating parent directories as needed.
fn write_file(dir: &PathBuf, rel: &str, contents: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(&path, contents).expect("write file");
}

/// Find a result whose `name` equals `name`.
fn find_named<'a>(results: &'a [serde_json::Value], name: &str) -> Option<&'a serde_json::Value> {
    results.iter().find(|v| {
        v.get("name").and_then(|n| n.as_str()) == Some(name)
    })
}

#[test]
fn workspace_symbols_finds_arrow_by_name() {
    let dir = unique_temp_dir("arrow-by-name");
    // A general .hnl file whose top-level binding is named `insert`.
    write_file(
        &dir,
        "arrows/collection/insert.hnl",
        "insert = 1\n",
    );

    let results = workspace_symbols("insert", Some(dir.to_str().unwrap()));

    assert!(
        find_named(&results, "insert").is_some(),
        "expected to find 'insert', got: {:?}",
        results
    );
}

#[test]
#[ignore = "kit lexer (old grammar) does not recognise `law` as a keyword, so \
            document_symbols cannot detect contract files; needs lexer v0.1 update"]
fn workspace_symbols_finds_law_in_contracts() {
    let dir = unique_temp_dir("law-contracts");
    write_file(
        &dir,
        "contracts/prim.hnl",
        "law prim.add : a ~> b\n",
    );

    let results = workspace_symbols("prim.add", Some(dir.to_str().unwrap()));

    assert!(
        find_named(&results, "prim.add").is_some(),
        "expected to find 'prim.add', got: {:?}",
        results
    );
}

#[test]
fn workspace_symbols_empty_query_returns_all() {
    let dir = unique_temp_dir("empty-query");
    write_file(&dir, "arrows/collection/insert.hnl", "insert = 1\n");
    write_file(&dir, "arrows/fn/map.hnl", "map = 1\n");

    let results = workspace_symbols("", Some(dir.to_str().unwrap()));

    // We expect at least the two arrow symbols we authored.
    assert!(
        find_named(&results, "insert").is_some(),
        "missing 'insert': {:?}",
        results
    );
    assert!(
        find_named(&results, "map").is_some(),
        "missing 'map': {:?}",
        results
    );
}

#[test]
fn workspace_symbols_no_match_returns_empty() {
    let dir = unique_temp_dir("no-match");
    write_file(&dir, "arrows/collection/insert.hnl", "insert = 1\n");

    let results = workspace_symbols("does-not-exist", Some(dir.to_str().unwrap()));

    assert!(results.is_empty(), "expected no results, got: {:?}", results);
}

#[test]
fn workspace_symbols_case_insensitive() {
    let dir = unique_temp_dir("case-insensitive");
    write_file(&dir, "arrows/collection/insert.hnl", "insert = 1\n");

    let results = workspace_symbols("INSERT", Some(dir.to_str().unwrap()));

    assert!(
        find_named(&results, "insert").is_some(),
        "case-insensitive search for 'INSERT' should find 'insert': {:?}",
        results
    );
}

#[test]
fn workspace_symbols_includes_container_name() {
    let dir = unique_temp_dir("container-name");
    write_file(&dir, "arrows/collection/insert.hnl", "insert = 1\n");
    write_file(&dir, "arrows/fn/map.hnl", "map = 1\n");

    let results = workspace_symbols("", Some(dir.to_str().unwrap()));

    let insert = find_named(&results, "insert").expect("found insert");
    let container = insert
        .get("containerName")
        .and_then(|c| c.as_str())
        .expect("containerName present");
    assert_eq!(
        container, "collection",
        "insert containerName should be 'collection': {:?}",
        insert
    );

    let map = find_named(&results, "map").expect("found map");
    let container = map
        .get("containerName")
        .and_then(|c| c.as_str())
        .expect("containerName present");
    assert_eq!(
        container, "fn",
        "map containerName should be 'fn': {:?}",
        map
    );
}

#[test]
fn workspace_symbols_returns_valid_json() {
    let dir = unique_temp_dir("valid-json");
    write_file(&dir, "arrows/collection/insert.hnl", "insert = 1\n");

    let results = workspace_symbols("insert", Some(dir.to_str().unwrap()));

    assert!(!results.is_empty(), "expected at least one result");
    for r in &results {
        assert!(r.get("name").and_then(|n| n.as_str()).is_some(), "missing name: {:?}", r);
        assert!(r.get("kind").and_then(|k| k.as_u64()).is_some(), "missing kind: {:?}", r);
        let location = r.get("location").expect("missing location");
        assert!(
            location.get("uri").and_then(|u| u.as_str()).is_some(),
            "missing location.uri: {:?}",
            r
        );
        let range = location.get("range").expect("missing location.range");
        assert!(range.get("start").is_some(), "missing range.start: {:?}", r);
        assert!(range.get("end").is_some(), "missing range.end: {:?}", r);
        assert!(
            r.get("containerName").and_then(|c| c.as_str()).is_some(),
            "missing containerName: {:?}",
            r
        );
    }
}
