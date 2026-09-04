//! Tests for `crate::cross_file_rename`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cross_file_rename::{prepare_rename, rename};

/// Monotonic counter so each test gets a unique temp directory, even when
/// tests run in parallel (where `SystemTime::now()` can return the same
/// nanosecond on separate threads).
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a unique temporary directory for a test, returning its path.
fn make_temp_dir(prefix: &str) -> PathBuf {
    let n = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!(
        "handl-lsp-rename-{}-{}-{}",
        prefix,
        std::process::id(),
        n,
    ));
    fs::create_dir_all(&base).expect("create temp dir");
    base
}

#[test]
fn prepare_rename_on_identifier_returns_name() {
    // `x` is followed by `=`, so it is a renameable binding.
    let source = "x = 42";
    let result = prepare_rename(source, 0);
    let result = result.expect("expected a prepareRename result for `x`");
    assert_eq!(result["placeholder"].as_str().unwrap(), "x");
    assert!(result.get("range").is_some(), "result should include a range");
}

#[test]
fn prepare_rename_on_whitespace_returns_none() {
    // Offset 1 is the space between `x` and `=` — no identifier there.
    let source = "x = 42";
    assert!(prepare_rename(source, 1).is_none());
}

#[test]
fn prepare_rename_on_property_access_returns_none() {
    // `acc` is preceded by `.` (property access), so it is not renameable.
    let source = "state.acc";
    let acc_offset = source.find("acc").unwrap();
    assert!(prepare_rename(source, acc_offset).is_none());
}

#[test]
fn rename_updates_references_in_current_file() {
    // `x` is defined and used; renaming to `y` should edit both occurrences
    // in the current (untitled) file.
    let source = "x = 42\nx";
    let result = rename(source, 0, "y", None).expect("expected a rename result");
    let changes = result["documentChanges"]
        .as_array()
        .expect("documentChanges should be an array");
    assert!(!changes.is_empty(), "expected at least one document change");

    // The first document change should be a TextDocumentEdit for the
    // current file with newText `y`.
    let first = &changes[0];
    assert!(first.get("edits").is_some(), "expected a TextDocumentEdit");
    let edits = first["edits"].as_array().unwrap();
    assert!(!edits.is_empty());
    assert!(
        edits.iter().all(|e| e["newText"].as_str() == Some("y")),
        "all edits should replace with `y`: {:?}",
        edits
    );
}

#[test]
fn rename_updates_references_across_files() {
    let dir = make_temp_dir("across");
    let arrows_root = dir.join("handl-packages").join("kit-stdlib");
    let fn_dir = arrows_root.join("fn");
    let collection_dir = arrows_root.join("collection");
    fs::create_dir_all(&fn_dir).expect("create arrows/fn");
    fs::create_dir_all(&collection_dir).expect("create arrows/collection");

    // The arrow file being referenced.
    fs::write(fn_dir.join("insert.hnl"), "-- insert arrow\n").expect("write insert.hnl");
    // Current file references `insert`.
    let sort_path = collection_dir.join("sort.hnl");
    let sort_source = "insert(input)";
    fs::write(&sort_path, sort_source).expect("write sort.hnl");
    // Another file also references `insert`.
    let each_path = collection_dir.join("each.hnl");
    fs::write(&each_path, "insert(input)").expect("write each.hnl");

    let file_path = sort_path.to_string_lossy().to_string();
    let result = rename(sort_source, 0, "newinsert", Some(&file_path))
        .expect("expected a rename result");

    let changes = result["documentChanges"]
        .as_array()
        .expect("documentChanges should be an array");

    // Collect URIs that received text edits.
    let edited_uris: Vec<String> = changes
        .iter()
        .filter(|c| c.get("edits").is_some())
        .map(|c| c["textDocument"]["uri"].as_str().unwrap().to_string())
        .collect();
    assert!(
        edited_uris.iter().any(|u| u.ends_with("sort.hnl")),
        "expected an edit for sort.hnl: {:?}",
        edited_uris
    );
    assert!(
        edited_uris.iter().any(|u| u.ends_with("each.hnl")),
        "expected an edit for each.hnl: {:?}",
        edited_uris
    );

    // All text edits should use the new name.
    for change in changes {
        if let Some(edits) = change.get("edits").and_then(|e| e.as_array()) {
            for e in edits {
                assert_eq!(e["newText"].as_str().unwrap(), "newinsert");
            }
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rename_renames_arrow_file() {
    let dir = make_temp_dir("filerename");
    let arrows_root = dir.join("handl-packages").join("kit-stdlib");
    let fn_dir = arrows_root.join("fn");
    let collection_dir = arrows_root.join("collection");
    fs::create_dir_all(&fn_dir).expect("create arrows/fn");
    fs::create_dir_all(&collection_dir).expect("create arrows/collection");

    fs::write(fn_dir.join("insert.hnl"), "-- insert arrow\n").expect("write insert.hnl");
    let sort_path = collection_dir.join("sort.hnl");
    fs::write(&sort_path, "insert(input)").expect("write sort.hnl");

    let file_path = sort_path.to_string_lossy().to_string();
    let result = rename("insert(input)", 0, "newinsert", Some(&file_path))
        .expect("expected a rename result");

    let changes = result["documentChanges"]
        .as_array()
        .expect("documentChanges should be an array");
    let has_rename_file = changes.iter().any(|c| c["kind"].as_str() == Some("rename"));
    assert!(
        has_rename_file,
        "expected a RenameFile resource in documentChanges: {:?}",
        changes
    );

    // Verify the RenameFile points at insert.hnl → newinsert.hnl.
    let rename_entry = changes
        .iter()
        .find(|c| c["kind"].as_str() == Some("rename"))
        .unwrap();
    assert!(
        rename_entry["oldUri"]
            .as_str()
            .unwrap()
            .ends_with("insert.hnl"),
        "oldUri should be insert.hnl: {:?}",
        rename_entry
    );
    assert!(
        rename_entry["newUri"]
            .as_str()
            .unwrap()
            .ends_with("newinsert.hnl"),
        "newUri should be newinsert.hnl: {:?}",
        rename_entry
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rename_non_arrow_identifier_no_file_rename() {
    // `x` is a binding, not an arrow file reference, and no file_path is
    // given — so there should be no RenameFile resource.
    let source = "x = 42\nx";
    let result = rename(source, 0, "y", None).expect("expected a rename result");
    let changes = result["documentChanges"]
        .as_array()
        .expect("documentChanges should be an array");
    let has_rename_file = changes.iter().any(|c| c["kind"].as_str() == Some("rename"));
    assert!(
        !has_rename_file,
        "non-arrow identifier should not trigger a file rename: {:?}",
        changes
    );
}

#[test]
fn rename_returns_valid_workspace_edit_json() {
    let dir = make_temp_dir("validjson");
    let arrows_root = dir.join("handl-packages").join("kit-stdlib");
    let fn_dir = arrows_root.join("fn");
    let collection_dir = arrows_root.join("collection");
    fs::create_dir_all(&fn_dir).expect("create arrows/fn");
    fs::create_dir_all(&collection_dir).expect("create arrows/collection");

    fs::write(fn_dir.join("insert.hnl"), "-- insert arrow\n").expect("write insert.hnl");
    let sort_path = collection_dir.join("sort.hnl");
    fs::write(&sort_path, "insert(input)").expect("write sort.hnl");

    let file_path = sort_path.to_string_lossy().to_string();
    let result = rename("insert(input)", 0, "newinsert", Some(&file_path))
        .expect("expected a rename result");

    // Top-level shape: { documentChanges: [...] }
    assert!(result.get("documentChanges").is_some());
    let changes = result["documentChanges"]
        .as_array()
        .expect("documentChanges should be an array");
    assert!(!changes.is_empty());

    for change in changes {
        if change.get("edits").is_some() {
            // TextDocumentEdit: { textDocument: { uri, version }, edits: [...] }
            assert!(
                change["textDocument"].get("uri").is_some(),
                "TextDocumentEdit missing uri: {:?}",
                change
            );
            let edits = change["edits"]
                .as_array()
                .expect("edits should be an array");
            for e in edits {
                assert!(e.get("range").is_some(), "edit missing range: {:?}", e);
                assert!(e.get("newText").is_some(), "edit missing newText: {:?}", e);
            }
        } else if change["kind"].as_str() == Some("rename") {
            // RenameFile: { kind, oldUri, newUri }
            assert!(change.get("oldUri").is_some(), "RenameFile missing oldUri");
            assert!(change.get("newUri").is_some(), "RenameFile missing newUri");
        } else {
            panic!("unexpected documentChange entry: {:?}", change);
        }
    }

    let _ = fs::remove_dir_all(&dir);
}
