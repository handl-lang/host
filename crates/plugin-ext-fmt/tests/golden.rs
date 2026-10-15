//! Golden-file tests for the canonical formatter (ADR-0086).
//!
//! Each golden pair is `*.input.handl` (source) and `*.expected.handl`
//! (canonical output). The test discovers all pairs dynamically.

use std::fs;
use std::path::PathBuf;

use handl_plugin_ext_fmt::format_source_with_language;

/// The canonical language, assembled once from the dev-dep data crate
/// (integration tests may depend on `lang/` data; the library must not).
fn lang() -> &'static handl_language::Language {
    static LANG: std::sync::OnceLock<handl_language::Language> =
        std::sync::OnceLock::new();
    LANG.get_or_init(|| {
        handl_kit_data_canonical::language_from_yaml()
            .expect("canonical language assembles")
    })
}

fn format_source(src: &str) -> Result<String, handl_plugin_ext_fmt::FormatError> {
    format_source_with_language(lang(), src)
}

/// Find all `.input.handl` files in the `tests/golden/` directory and return
/// their stem names (e.g. `binding`, `import`, …).
fn golden_names() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(stem) = name.strip_suffix(".input.handl") {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

/// Read a golden file by stem and suffix.
fn read_golden(stem: &str, suffix: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{stem}.{suffix}"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

#[test]
fn golden_file_tests() {
    let names = golden_names();
    assert!(!names.is_empty(), "no golden files found in tests/golden/");
    for name in &names {
        let input = read_golden(name, "input.handl");
        let expected = read_golden(name, "expected.handl");
        let actual = format_source(&input)
            .unwrap_or_else(|e| panic!("format_source failed for golden '{name}': {e}"));
        assert_eq!(
            actual, expected,
            "golden mismatch for '{name}':\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
        );
    }
}

#[test]
fn golden_idempotence() {
    let names = golden_names();
    for name in &names {
        let input = read_golden(name, "input.handl");
        let once = format_source(&input)
            .unwrap_or_else(|e| panic!("first format_source failed for '{name}': {e}"));
        let twice = format_source(&once)
            .unwrap_or_else(|e| panic!("second format_source failed for '{name}': {e}"));
        assert_eq!(
            once, twice,
            "idempotence violated for golden '{name}':\n--- once ---\n{once}\n--- twice ---\n{twice}"
        );
    }
}

#[test]
fn golden_expected_is_idempotent() {
    // The expected output itself must be a fixpoint.
    let names = golden_names();
    for name in &names {
        let expected = read_golden(name, "expected.handl");
        let reformatted = format_source(&expected)
            .unwrap_or_else(|e| panic!("format_source failed for expected '{name}': {e}"));
        assert_eq!(
            expected, reformatted,
            "expected output for '{name}' is not a fixpoint"
        );
    }
}
