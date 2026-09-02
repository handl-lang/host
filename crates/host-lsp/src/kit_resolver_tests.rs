//! Tests for the kit resolver (ADR 0003).

use super::kit_resolver::{
    KitResolver, ResolvedKit, TomlManifest, DEFAULT_KIT_NAME, parse_toml,
};

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

/// Write a `.handl.toml` with the given contents into `dir`.
fn write_manifest(dir: &PathBuf, contents: &str) {
    fs::write(dir.join(".handl.toml"), contents).unwrap();
}

/// Create a `data/` directory inside `dir` to simulate a kit data directory.
fn make_data_dir(dir: &PathBuf) -> PathBuf {
    let data = dir.join("data");
    fs::create_dir_all(&data).unwrap();
    data
}

// ---------------------------------------------------------------------------
// Filesystem resolution tests
// ---------------------------------------------------------------------------

#[test]
fn resolve_finds_handl_toml_in_same_dir() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    make_data_dir(&dir);
    write_manifest(
        &dir,
        r#"[language]
kit = "./data"
"#,
    );

    let resolver = KitResolver::new();
    let file = dir.join("sort.hnl");
    let resolved = resolver.resolve(file.to_str().unwrap());

    assert!(!resolved.is_default());
    assert_eq!(resolved.version, None);
    assert!(resolved.kit_path.ends_with("data"), "got {:?}", resolved.kit_path);
    assert_eq!(resolved.manifest_dir.as_deref(), Some(dir.as_path()));
}

#[test]
fn resolve_finds_handl_toml_in_parent_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    make_data_dir(&root);
    write_manifest(
        &root,
        r#"[language]
kit = "./data"
"#,
    );

    let sub = root.join("arrows");
    fs::create_dir_all(&sub).unwrap();

    let resolver = KitResolver::new();
    let file = sub.join("sort.hnl");
    let resolved = resolver.resolve(file.to_str().unwrap());

    assert!(!resolved.is_default());
    assert_eq!(resolved.manifest_dir.as_deref(), Some(root.as_path()));
    assert!(resolved.kit_path.ends_with("data"));
}

#[test]
fn resolve_finds_nearest_manifest_not_furthest() {
    let tmp = TempDir::new().unwrap();
    let outer = tmp.path().to_path_buf();
    let outer_data = make_data_dir(&outer);
    write_manifest(
        &outer,
        r#"[language]
kit = "./data"
version = "0.1"
"#,
    );

    let inner = outer.join("sub");
    fs::create_dir_all(&inner).unwrap();
    let inner_data = make_data_dir(&inner);
    write_manifest(
        &inner,
        r#"[language]
kit = "./data"
version = "0.2"
"#,
    );

    let resolver = KitResolver::new();
    let file = inner.join("file.hnl");
    let resolved = resolver.resolve(file.to_str().unwrap());

    // The nearest (inner) manifest wins.
    assert_eq!(resolved.manifest_dir.as_deref(), Some(inner.as_path()));
    assert_eq!(resolved.version.as_deref(), Some("0.2"));
    assert_eq!(resolved.kit_path, inner_data);
    assert_ne!(resolved.kit_path, outer_data);
}

#[test]
fn resolve_falls_back_to_default_when_no_manifest() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    let file = dir.join("sort.hnl");

    let resolver = KitResolver::new();
    let resolved = resolver.resolve(file.to_str().unwrap());

    assert!(resolved.is_default());
    assert_eq!(resolved.kit_path, PathBuf::from(DEFAULT_KIT_NAME));
    assert_eq!(resolved.version, None);
    assert_eq!(resolved.manifest_dir, None);
}

#[test]
fn resolve_caches_results() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    make_data_dir(&dir);
    write_manifest(
        &dir,
        r#"[language]
kit = "./data"
"#,
    );

    let resolver = KitResolver::new();
    let file = dir.join("a.hnl");

    let first = resolver.resolve(file.to_str().unwrap());
    let second = resolver.resolve(file.to_str().unwrap());

    // Both calls return the same resolved kit (cached).
    assert_eq!(first, second);
    assert!(!first.is_default());
    assert_eq!(first.manifest_dir.as_deref(), Some(dir.as_path()));
}

#[test]
fn resolve_handles_kit_as_path() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let custom = root.join("custom-kit");
    fs::create_dir_all(custom.join("data")).unwrap();
    write_manifest(
        &root,
        r#"[language]
kit = "./custom-kit/data"
"#,
    );

    let resolver = KitResolver::new();
    let file = root.join("ingest.hnl");
    let resolved = resolver.resolve(file.to_str().unwrap());

    assert!(!resolved.is_default());
    assert!(resolved.kit_path.ends_with("custom-kit/data"));
}

#[test]
fn resolve_handles_kit_as_crate_name() {
    let tmp = TempDir::new().unwrap();
    // Build a fake workspace: <tmp>/workspace with Cargo.toml and crates/<name>/data
    let workspace = tmp.path().join("workspace");
    let crates_dir = workspace.join("crates");
    let kit_data = crates_dir.join("handl-kit-data-canonical").join("data");
    fs::create_dir_all(&kit_data).unwrap();
    fs::write(workspace.join("Cargo.toml"), "[workspace]\n").unwrap();

    // The project lives under workspace/project-a with a manifest.
    let project = workspace.join("project-a");
    fs::create_dir_all(&project).unwrap();
    write_manifest(
        &project,
        r#"[language]
kit = "handl-kit-data-canonical"
"#,
    );

    let resolver = KitResolver::new();
    let file = project.join("sort.hnl");
    let resolved = resolver.resolve(file.to_str().unwrap());

    assert!(!resolved.is_default());
    assert!(resolved.kit_path.ends_with("crates/handl-kit-data-canonical/data"));
}

#[test]
fn resolve_handles_missing_kit_field() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    write_manifest(
        &dir,
        r#"[language]
version = "0.1"
"#,
    );

    let resolver = KitResolver::new();
    let file = dir.join("sort.hnl");
    let resolved = resolver.resolve(file.to_str().unwrap());

    // No kit field -> default kit, but the manifest was still found.
    assert_eq!(resolved.kit_path, PathBuf::from(DEFAULT_KIT_NAME));
}

#[test]
fn resolve_empty_path_returns_default() {
    let resolver = KitResolver::new();
    let resolved = resolver.resolve("");
    assert!(resolved.is_default());
    assert_eq!(resolved.kit_path, PathBuf::from(DEFAULT_KIT_NAME));
}

// ---------------------------------------------------------------------------
// TOML parser tests
// ---------------------------------------------------------------------------

#[test]
fn parse_toml_parses_language_section() {
    let manifest = parse_toml(
        r#"[language]
kit = "handl-kit-data-canonical"
"#,
    );
    let lang = manifest.language.expect("language section");
    assert_eq!(lang.kit.as_deref(), Some("handl-kit-data-canonical"));
    assert_eq!(lang.version, None);
}

#[test]
fn parse_toml_parses_version() {
    let manifest = parse_toml(
        r#"[language]
kit = "handl-kit-data-canonical"
version = "0.1"
"#,
    );
    let lang = manifest.language.expect("language section");
    assert_eq!(lang.kit.as_deref(), Some("handl-kit-data-canonical"));
    assert_eq!(lang.version.as_deref(), Some("0.1"));
}

#[test]
fn parse_toml_ignores_comments() {
    let manifest = parse_toml(
        r#"# top-level comment
[language] # section comment
# another comment
kit = "handl-kit-data-canonical"    # trailing comment
version = "0.2" # pin
"#,
    );
    let lang = manifest.language.expect("language section");
    assert_eq!(lang.kit.as_deref(), Some("handl-kit-data-canonical"));
    assert_eq!(lang.version.as_deref(), Some("0.2"));
}

#[test]
fn parse_toml_empty_file_returns_empty() {
    let manifest = parse_toml("");
    assert_eq!(manifest, TomlManifest::default());
    assert!(manifest.language.is_none());
}

#[test]
fn parse_toml_ignores_unknown_sections() {
    let manifest = parse_toml(
        r#"[editor]
enableInlayHints = "true"
[language]
kit = "my-kit"
"#,
    );
    let lang = manifest.language.expect("language section");
    assert_eq!(lang.kit.as_deref(), Some("my-kit"));
}

#[test]
fn parse_toml_handles_single_quoted_strings() {
    let manifest = parse_toml(
        r#"[language]
kit = 'single-quoted'
"#,
    );
    assert_eq!(
        manifest.language.unwrap().kit.as_deref(),
        Some("single-quoted")
    );
}

#[test]
fn parse_toml_preserves_hash_inside_strings() {
    let manifest = parse_toml(
        r#"[language]
kit = "has#hash"
"#,
    );
    assert_eq!(manifest.language.unwrap().kit.as_deref(), Some("has#hash"));
}

// ---------------------------------------------------------------------------
// ResolvedKit helpers
// ---------------------------------------------------------------------------

#[test]
fn default_kit_is_default() {
    let kit = ResolvedKit::default_kit();
    assert!(kit.is_default());
    assert_eq!(kit.kit_path, PathBuf::from(DEFAULT_KIT_NAME));
}
