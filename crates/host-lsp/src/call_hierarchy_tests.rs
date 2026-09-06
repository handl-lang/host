//! Tests for `crate::call_hierarchy`.
//!
//! The cross-file tests (`outgoing_calls_*`, `incoming_calls_*`) build a
//! temporary `arrows/` tree on disk so that `find_arrows_root` can locate the
//! root and the call-hierarchy logic can scan sibling `.hnl` files.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::call_hierarchy::{incoming_calls, outgoing_calls, prepare_call_hierarchy};

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
        "handl-lsp-call-hierarchy-{}-{}-{}",
        prefix,
        std::process::id(),
        n,
    ));
    fs::create_dir_all(&base).expect("create temp dir");
    base
}

/// Write a `.hnl` file under `arrows_root/subdir/name.hnl` with `contents`,
/// returning the file's filesystem path.
fn write_arrow(arrows_root: &PathBuf, subdir: &str, name: &str, contents: &str) -> PathBuf {
    let dir = arrows_root.join(subdir);
    fs::create_dir_all(&dir).expect("create arrow subdir");
    let path = dir.join(format!("{}.hnl", name));
    fs::write(&path, contents).expect("write .hnl file");
    path
}

// ---------------------------------------------------------------------------
// prepare_call_hierarchy
// ---------------------------------------------------------------------------

#[test]
fn prepare_call_hierarchy_returns_item_for_named_arrow() {
    // A named arrow: `reduce = loop(arr("fold_step") >>> first(id))`.
    // The cursor sits on the `reduce` identifier.
    let source = "reduce = loop(arr(\"fold_step\") >>> first(id))";
    let offset = 3; // inside `reduce`

    let items = prepare_call_hierarchy(source, offset, None);
    assert_eq!(items.len(), 1, "expected exactly one call hierarchy item");

    let item = &items[0];
    assert_eq!(item["name"].as_str().unwrap(), "reduce");
    assert_eq!(
        item["kind"].as_u64().unwrap(),
        12,
        "kind should be Function (12)"
    );
    assert_eq!(item["detail"].as_str().unwrap(), "arrow");
    // selectionRange should cover the `reduce` identifier (offsets 0..6).
    assert_eq!(item["selectionRange"]["start"]["line"].as_u64().unwrap(), 0);
    assert_eq!(
        item["selectionRange"]["start"]["character"]
            .as_u64()
            .unwrap(),
        0
    );
    assert_eq!(
        item["selectionRange"]["end"]["character"].as_u64().unwrap(),
        6
    );
}

#[test]
fn prepare_call_hierarchy_returns_item_for_unnamed_file_stem_arrow() {
    // An unnamed arrow file: the whole file is one arrow whose name is the
    // file stem. The cursor sits on an identifier matching the file stem.
    let source = "sort >>> id";
    let offset = source.find("sort").unwrap();

    let file_path = "/tmp/handl-lsp-call-hierarchy-unnamed/arrows/collection/sort.hnl";
    let items = prepare_call_hierarchy(source, offset, Some(file_path));
    assert_eq!(items.len(), 1, "expected one item for the file-stem arrow");
    assert_eq!(items[0]["name"].as_str().unwrap(), "sort");
    assert_eq!(items[0]["kind"].as_u64().unwrap(), 12);
}

#[test]
fn prepare_call_hierarchy_returns_empty_for_non_arrow() {
    // The cursor is on `insert`, which is a reference (not a definition) in
    // this file. The file's only arrow is the unnamed file-stem arrow `sort`,
    // so `insert` does not match any arrow name → empty.
    let source = "loop(insert >>> id)";
    let offset = source.find("insert").unwrap();
    let file_path = "/tmp/handl-lsp-call-hierarchy-non-arrow/arrows/collection/sort.hnl";

    let items = prepare_call_hierarchy(source, offset, Some(file_path));
    assert!(
        items.is_empty(),
        "expected no items for a non-arrow identifier, got {:?}",
        items
    );
}

#[test]
fn prepare_call_hierarchy_returns_empty_when_cursor_not_on_identifier() {
    // Cursor on the `=` operator of a named arrow — not an identifier.
    let source = "reduce = loop(id)";
    let offset = source.find('=').unwrap();
    let items = prepare_call_hierarchy(source, offset, None);
    assert!(items.is_empty(), "expected empty when not on an identifier");
}

// ---------------------------------------------------------------------------
// outgoing_calls
// ---------------------------------------------------------------------------

#[test]
fn outgoing_calls_finds_referenced_arrows() {
    let dir = make_temp_dir("outgoing");
    let arrows_root = dir.join("handl-packages");

    // `insert.hnl` is a separate arrow file under the same root.
    write_arrow(
        &arrows_root,
        "collection",
        "insert",
        "-- insert arrow\nloop(id)\n",
    );

    // The current file `sort.hnl` is an unnamed arrow whose body references
    // `insert` as an identifier.
    let sort_path = write_arrow(&arrows_root, "collection", "sort", "loop(insert >>> id)\n");
    let sort_str = sort_path.to_string_lossy().to_string();

    // Outgoing calls for the `sort` arrow (named by its file stem).
    let calls = outgoing_calls("sort", Some(&sort_str));

    assert_eq!(
        calls.len(),
        1,
        "expected one outgoing call to insert, got {:?}",
        calls
    );
    let call = &calls[0];
    assert_eq!(call["to"]["name"].as_str().unwrap(), "insert");
    assert_eq!(call["to"]["kind"].as_u64().unwrap(), 12);
    assert!(
        call["to"]["uri"].as_str().unwrap().ends_with("insert.hnl"),
        "to.uri should point at insert.hnl: {}",
        call["to"]["uri"]
    );
    // fromRanges should contain at least one range covering the `insert` ref.
    let ranges = call["fromRanges"].as_array().unwrap();
    assert!(!ranges.is_empty(), "expected at least one fromRange");
    let first = &ranges[0];
    assert_eq!(first["start"]["line"].as_u64().unwrap(), 0);
    // `insert` starts at offset 5 ("loop(" is 5 chars).
    assert_eq!(first["start"]["character"].as_u64().unwrap(), 5);
}

#[test]
fn outgoing_calls_finds_string_literal_references() {
    // A named arrow that references another arrow via `arr("name")`.
    let dir = make_temp_dir("outgoing-str");
    let arrows_root = dir.join("handl-packages");
    write_arrow(&arrows_root, "fn", "fold_step", "-- fold_step arrow\nid\n");

    let reduce_path = write_arrow(
        &arrows_root,
        "fn",
        "reduce",
        "reduce = loop(arr(\"fold_step\") >>> first(id))\n",
    );
    let reduce_str = reduce_path.to_string_lossy().to_string();

    // Outgoing calls for the named `reduce` arrow.
    let calls = outgoing_calls("reduce", Some(&reduce_str));

    assert_eq!(
        calls.len(),
        1,
        "expected one outgoing call to fold_step, got {:?}",
        calls
    );
    assert_eq!(calls[0]["to"]["name"].as_str().unwrap(), "fold_step");
}

#[test]
fn outgoing_calls_no_references_returns_empty() {
    let dir = make_temp_dir("outgoing-none");
    let arrows_root = dir.join("handl-packages");

    // `insert.hnl` exists but is not referenced by `sort.hnl`.
    write_arrow(
        &arrows_root,
        "collection",
        "insert",
        "-- insert arrow\nloop(id)\n",
    );
    let sort_path = write_arrow(&arrows_root, "collection", "sort", "loop(id)\n");
    let sort_str = sort_path.to_string_lossy().to_string();

    // `sort` references no other arrow → empty.
    let calls = outgoing_calls("sort", Some(&sort_str));
    assert!(
        calls.is_empty(),
        "expected no outgoing calls, got {:?}",
        calls
    );
}

#[test]
fn outgoing_calls_excludes_self_reference() {
    // `insert.hnl` references itself recursively; the self-reference should
    // not appear as an outgoing call.
    let dir = make_temp_dir("outgoing-self");
    let arrows_root = dir.join("handl-packages");
    let insert_path = write_arrow(
        &arrows_root,
        "collection",
        "insert",
        "loop(insert >>> id)\n",
    );
    let insert_str = insert_path.to_string_lossy().to_string();

    // `insert` references only itself → no outgoing calls to *other* arrows.
    let calls = outgoing_calls("insert", Some(&insert_str));
    assert!(
        calls.is_empty(),
        "self-references should be excluded, got {:?}",
        calls
    );
}

#[test]
fn outgoing_calls_without_arrows_root_returns_empty() {
    // No `arrows/` segment in the path → no arrows root → empty.
    let calls = outgoing_calls("sort", Some("/tmp/no-such-arrows-root/sort.hnl"));
    assert!(calls.is_empty());
}

// ---------------------------------------------------------------------------
// incoming_calls
// ---------------------------------------------------------------------------

#[test]
fn incoming_calls_finds_callers() {
    let dir = make_temp_dir("incoming");
    let arrows_root = dir.join("handl-packages");

    // `insert.hnl` is the target arrow.
    write_arrow(
        &arrows_root,
        "collection",
        "insert",
        "-- insert arrow\nloop(id)\n",
    );

    // `sort.hnl` references `insert` in its body — it is a caller.
    let sort_path = write_arrow(&arrows_root, "collection", "sort", "loop(insert >>> id)\n");
    let sort_str = sort_path.to_string_lossy().to_string();

    // Find callers of `insert`; `sort` should be the only one.
    let calls = incoming_calls("insert", Some(&sort_str));

    assert_eq!(
        calls.len(),
        1,
        "expected exactly one incoming caller (sort), got {:?}",
        calls
    );
    let call = &calls[0];
    assert_eq!(call["from"]["name"].as_str().unwrap(), "sort");
    assert_eq!(call["from"]["kind"].as_u64().unwrap(), 12);
    assert!(
        call["from"]["uri"].as_str().unwrap().ends_with("sort.hnl"),
        "from.uri should point at sort.hnl: {}",
        call["from"]["uri"]
    );
    let ranges = call["fromRanges"].as_array().unwrap();
    assert!(!ranges.is_empty(), "expected at least one fromRange");
    // The `insert` reference starts at offset 5 in "loop(insert >>> id)\n".
    assert_eq!(ranges[0]["start"]["character"].as_u64().unwrap(), 5);
}

#[test]
fn incoming_calls_excludes_target_itself() {
    // `insert.hnl` references itself; it should not appear as its own caller.
    let dir = make_temp_dir("incoming-self");
    let arrows_root = dir.join("handl-packages");
    let insert_path = write_arrow(
        &arrows_root,
        "collection",
        "insert",
        "loop(insert >>> id)\n",
    );
    let insert_str = insert_path.to_string_lossy().to_string();

    let calls = incoming_calls("insert", Some(&insert_str));
    assert!(
        calls.is_empty(),
        "target should not be its own caller, got {:?}",
        calls
    );
}

#[test]
fn incoming_calls_no_callers_returns_empty() {
    let dir = make_temp_dir("incoming-none");
    let arrows_root = dir.join("handl-packages");
    // `insert.hnl` exists but nothing references it.
    write_arrow(&arrows_root, "collection", "insert", "loop(id)\n");
    // `sort.hnl` does not reference insert.
    let sort_path = write_arrow(&arrows_root, "collection", "sort", "loop(id)\n");
    let sort_str = sort_path.to_string_lossy().to_string();

    let calls = incoming_calls("insert", Some(&sort_str));
    assert!(
        calls.is_empty(),
        "expected no incoming callers, got {:?}",
        calls
    );
}

#[test]
fn incoming_calls_without_arrows_root_returns_empty() {
    let calls = incoming_calls("insert", Some("/tmp/no-arrows/sort.hnl"));
    assert!(calls.is_empty());
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn empty_source_returns_empty() {
    assert!(prepare_call_hierarchy("", 0, None).is_empty());
    assert!(outgoing_calls("x", None).is_empty());
    assert!(incoming_calls("insert", None).is_empty());
}

#[test]
fn prepare_call_hierarchy_ignores_let_bindings_in_arrow_body() {
    // A rich-loop arrow body contains a let-binding `cmp = ...` that must NOT
    // be treated as a named-arrow definition. The only arrow here is the
    // unnamed file-stem arrow `insert`.
    let source = "rec {\n  elem: input.0,\n  acc: input.1\n} (\n  state ~> match state.acc {\n    []: return [state.elem],\n    [h, ...t]:\n      cmp = prim.compare(state.elem, h) in\n      insert({ elem: state.elem, acc: t })\n  }\n)\n";
    let file_path = "/tmp/handl-lsp-call-hierarchy-let/arrows/collection/insert.hnl";

    // Cursor on `cmp` (the let-binding name) — not an arrow definition.
    let offset = source.find("cmp").unwrap();
    let items = prepare_call_hierarchy(source, offset, Some(file_path));
    assert!(
        items.is_empty(),
        "let-binding `cmp` should not be an arrow definition, got {:?}",
        items
    );

    // Cursor on `insert` (a reference) — not the file-stem name `insert`...
    // actually `insert` IS the file stem, so this should return the item.
    let insert_offset = source.find("insert(").unwrap();
    let items = prepare_call_hierarchy(source, insert_offset, Some(file_path));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"].as_str().unwrap(), "insert");
}
