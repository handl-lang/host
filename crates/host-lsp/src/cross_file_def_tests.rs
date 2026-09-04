use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cross_file_def::{
    find_cross_file_definition, find_cross_file_definition_uri, resolve_definition,
};
use crate::refinement::find_arrows_root;

/// Monotonic counter so each test gets a unique temp directory, even when
/// tests run in parallel (where `SystemTime::now()` can return the same
/// nanosecond on separate threads).
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a unique temporary directory for a test, returning its path.
fn make_temp_dir(prefix: &str) -> PathBuf {
    let n = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!(
        "handl-lsp-cross-file-{}-{}-{}",
        prefix,
        std::process::id(),
        n,
    ));
    fs::create_dir_all(&base).expect("create temp dir");
    base
}

#[test]
fn find_cross_file_definition_finds_arrow_file() {
    let dir = make_temp_dir("find");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib").join("fn");
    fs::create_dir_all(&arrows_dir).expect("create arrows/fn dir");
    let insert_path = arrows_dir.join("insert.hnl");
    fs::write(&insert_path, "-- insert arrow\n").expect("write insert.hnl");

    // The "current file" lives somewhere under arrows/ so that
    // find_arrows_root can locate the root.
    let current_file = dir
        .join("handl-packages").join("kit-stdlib")
        .join("collection")
        .join("sort.hnl");
    fs::create_dir_all(current_file.parent().unwrap()).expect("create collection dir");
    fs::write(&current_file, "-- sort arrow\n").expect("write sort.hnl");

    let current_str = current_file.to_string_lossy().to_string();
    let found = find_cross_file_definition("insert", &current_str);
    assert!(found.is_some(), "expected to find insert.hnl");
    let uri = found.unwrap();
    assert!(uri.starts_with("file://"), "uri should be a file URI: {}", uri);
    assert!(
        uri.ends_with("insert.hnl"),
        "uri should point at insert.hnl: {}",
        uri
    );
}

#[test]
fn find_cross_file_definition_returns_none_for_nonexistent() {
    let dir = make_temp_dir("none");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib").join("fn");
    fs::create_dir_all(&arrows_dir).expect("create arrows/fn dir");

    let current_file = dir.join("handl-packages").join("kit-stdlib").join("sort.hnl");
    fs::write(&current_file, "-- sort arrow\n").expect("write sort.hnl");

    let current_str = current_file.to_string_lossy().to_string();
    let found = find_cross_file_definition("does_not_exist", &current_str);
    assert!(found.is_none(), "should not find a nonexistent arrow");
}

#[test]
fn find_arrows_root_locates_arrows_directory() {
    // Mirror the refinement_tests assertion to keep this module
    // self-documenting about its dependency on find_arrows_root.
    assert_eq!(
        find_arrows_root("/home/user/platform/handl-packages/kit-stdlib/collection/sort.hnl"),
        Some("/home/user/platform/handl-packages/kit-stdlib/".to_string())
    );
}

#[test]
fn find_cross_file_definition_uri_returns_location_json() {
    let dir = make_temp_dir("uri");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib").join("fn");
    fs::create_dir_all(&arrows_dir).expect("create arrows/fn dir");
    let map_path = arrows_dir.join("map.hnl");
    fs::write(&map_path, "-- map arrow\n").expect("write map.hnl");

    let current_file = dir.join("handl-packages").join("kit-stdlib").join("collection").join("each.hnl");
    fs::create_dir_all(current_file.parent().unwrap()).expect("create collection dir");
    fs::write(&current_file, "-- each arrow\n").expect("write each.hnl");

    let current_str = current_file.to_string_lossy().to_string();
    let loc = find_cross_file_definition_uri("map", &current_str)
        .expect("should find map.hnl location");

    assert_eq!(loc["uri"].as_str().unwrap().ends_with("map.hnl"), true);
    assert_eq!(loc["range"]["start"]["line"].as_u64().unwrap(), 0);
    assert_eq!(loc["range"]["start"]["character"].as_u64().unwrap(), 0);
    assert_eq!(loc["range"]["end"]["line"].as_u64().unwrap(), 0);
    assert_eq!(loc["range"]["end"]["character"].as_u64().unwrap(), 0);
}

#[test]
fn resolve_definition_falls_back_to_cross_file() {
    let dir = make_temp_dir("resolve");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib").join("fn");
    fs::create_dir_all(&arrows_dir).expect("create arrows/fn dir");
    let insert_path = arrows_dir.join("insert.hnl");
    fs::write(&insert_path, "-- insert arrow\n").expect("write insert.hnl");

    // The current file references `insert` but does not define it locally,
    // so resolve_definition should fall back to cross-file resolution.
    let current_file = dir.join("handl-packages").join("kit-stdlib").join("collection").join("sort.hnl");
    fs::create_dir_all(current_file.parent().unwrap()).expect("create collection dir");
    let source = "insert(input)";
    fs::write(&current_file, source).expect("write sort.hnl");

    let current_str = current_file.to_string_lossy().to_string();

    // Offset pointing at the `insert` identifier (offset 0).
    let loc = resolve_definition(source, 0, &current_str)
        .expect("should resolve insert cross-file");

    let uri = loc["uri"].as_str().unwrap();
    assert!(
        uri.ends_with("insert.hnl"),
        "resolved uri should point at insert.hnl: {}",
        uri
    );
    assert_eq!(loc["range"]["start"]["line"].as_u64().unwrap(), 0);
    assert_eq!(loc["range"]["start"]["character"].as_u64().unwrap(), 0);
}

#[test]
fn resolve_definition_prefers_in_file_definition() {
    let dir = make_temp_dir("infile");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib").join("fn");
    fs::create_dir_all(&arrows_dir).expect("create arrows/fn dir");
    // Create a cross-file target that should NOT be chosen because the
    // name is defined locally in the current file.
    let x_path = arrows_dir.join("x.hnl");
    fs::write(&x_path, "-- x arrow\n").expect("write x.hnl");

    let current_file = dir.join("handl-packages").join("kit-stdlib").join("main.hnl");
    fs::write(&current_file, "").expect("write main.hnl");

    // `x` is defined locally with `=`, so in-file definition wins.
    let source = "x = 42\nx";
    let current_str = current_file.to_string_lossy().to_string();

    // Offset at the usage of `x` on the second line (after "x = 42\n").
    let usage_offset = source.find("\nx").unwrap() + 1;
    let loc = resolve_definition(source, usage_offset, &current_str)
        .expect("should resolve in-file definition");

    // The in-file definition uri should point at the current file, not x.hnl.
    let uri = loc["uri"].as_str().unwrap();
    assert!(
        uri.ends_with("main.hnl"),
        "in-file definition should point at current file: {}",
        uri
    );
}

#[test]
fn find_cross_file_definition_finds_axiom_in_contracts() {
    let dir = make_temp_dir("axiom");
    let arrows_dir = dir.join("handl-packages").join("kit-stdlib");
    let contracts_dir = dir.join("handl-packages").join("kit-stdlib").join("contracts");
    fs::create_dir_all(&arrows_dir).expect("create arrows dir");
    fs::create_dir_all(&contracts_dir).expect("create contracts dir");

    let prim_path = contracts_dir.join("prim.hnl");
    fs::write(&prim_path, "-- contracts\naxiom prim.add\n").expect("write prim.hnl");

    let current_file = arrows_dir.join("sort.hnl");
    fs::write(&current_file, "-- sort\n").expect("write sort.hnl");

    let current_str = current_file.to_string_lossy().to_string();
    let found = find_cross_file_definition("prim.add", &current_str);
    assert!(found.is_some(), "expected to find axiom prim.add in contracts");
    let uri = found.unwrap();
    assert!(
        uri.ends_with("prim.hnl"),
        "axiom uri should point at prim.hnl: {}",
        uri
    );
}
