use crate::file_watcher::{
    OpenDocument, register_watched_files, watched_files_changed, watched_files_changed_for,
};

#[test]
fn watched_files_changed_filters_to_hnl() {
    // Non-.hnl files are dropped; .hnl URIs are returned.
    let uris = vec![
        "file:///repo/handl-packages/kit-stdlib/sort.hnl".to_string(),
        "file:///repo/README.md".to_string(),
        "file:///repo/handl-packages/kit-stdlib/map.hnl".to_string(),
    ];
    let affected = watched_files_changed(&uris);
    assert_eq!(
        affected,
        vec![
            "file:///repo/handl-packages/kit-stdlib/sort.hnl".to_string(),
            "file:///repo/handl-packages/kit-stdlib/map.hnl".to_string(),
        ]
    );
}

#[test]
fn watched_files_changed_for_finds_referencing_documents() {
    // `main.hnl` references `sort` (the changed file's stem) → affected.
    // `other.hnl` does not → not affected.
    let changed = vec!["file:///repo/handl-packages/kit-stdlib/sort.hnl".to_string()];
    let main_src = "result = sort ~> map";
    let other_src = "result = filter ~> reduce";
    let docs = vec![
        OpenDocument { uri: "file:///repo/handl-packages/kit-stdlib/main.hnl", source: main_src },
        OpenDocument { uri: "file:///repo/handl-packages/kit-stdlib/other.hnl", source: other_src },
    ];
    let affected = watched_files_changed_for(&changed, &docs);
    assert_eq!(
        affected,
        vec!["file:///repo/handl-packages/kit-stdlib/main.hnl".to_string()],
        "only the document referencing `sort` should be affected: {affected:?}"
    );
}

#[test]
fn watched_files_changed_for_excludes_self() {
    // A file changing itself should not be flagged via the reference path
    // (its own changes are handled by didChange/didSave).
    let changed = vec!["file:///repo/handl-packages/kit-stdlib/sort.hnl".to_string()];
    let src = "sort = sort ~> 1";
    let docs = vec![OpenDocument {
        uri: "file:///repo/handl-packages/kit-stdlib/sort.hnl",
        source: src,
    }];
    let affected = watched_files_changed_for(&changed, &docs);
    assert!(
        affected.is_empty(),
        "a file should not re-flag itself via references: {affected:?}"
    );
}

#[test]
fn register_watched_files_builds_glob_pattern() {
    let opts = register_watched_files("/repo/handl-packages/kit-stdlib");
    let watchers = opts
        .get("watchers")
        .and_then(|w| w.as_array())
        .expect("watchers array");
    assert_eq!(watchers.len(), 1);
    let pattern = watchers[0]["globPattern"].as_str().expect("globPattern");
    assert!(
        pattern.ends_with("/**/*.hnl"),
        "glob pattern should cover all .hnl files: {pattern}"
    );
    assert!(pattern.starts_with("/repo/handl-packages/kit-stdlib"), "pattern should be rooted at arrows root: {pattern}");
}

#[test]
fn register_watched_files_empty_root_uses_bare_glob() {
    let opts = register_watched_files("");
    let pattern = opts["watchers"][0]["globPattern"].as_str().unwrap();
    assert_eq!(pattern, "**/*.hnl");
}
