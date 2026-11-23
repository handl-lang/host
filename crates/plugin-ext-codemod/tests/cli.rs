//! CLI-surface tests for `hnl codemod` (PL-004 T-086): plan → apply →
//! revert over a real tempdir, exercising the kernel journal/manifest.

use handl_kit_data_prelude::assemble;
use handl_language::Language;
use handl_plugin_ext_codemod::codemod_command;

fn lang() -> Language {
    assemble().unwrap().lang().clone()
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

const SPEC: &str = r#"name: del-assign
version: 1
targets:
  - language: handl
    glob: "src/**/*.hnl"
    unparse: preserve
rules:
  - id: del
    match: "assign"
    rewrite: { delete: true }
"#;

/// A tempdir holding `src/a.hnl` and a `codemod.yaml` at its root —
/// the spec's parent becomes the store root (package-relative globs).
struct Fixture {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    spec: std::path::PathBuf,
    source: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = root.join("src/a.hnl");
    std::fs::write(&source, "A := 1\nB := 2\n").unwrap();
    let spec = root.join("codemod.yaml");
    std::fs::write(&spec, SPEC).unwrap();
    Fixture {
        _dir: dir,
        root,
        spec,
        source,
    }
}

#[test]
fn usage_errors() {
    let lang = lang();
    assert_eq!(codemod_command(&lang, &[]), 2);
    // An unknown leading word is treated as the spec path; a missing
    // file is a usage error.
    assert_eq!(codemod_command(&lang, &argv(&["bogus"])), 2);
    assert_eq!(codemod_command(&lang, &argv(&["revert"])), 2);
    assert_eq!(
        codemod_command(&lang, &argv(&["plan", "x.yaml", "--bogus"])),
        2
    );
}

#[test]
fn plan_apply_revert_roundtrip() {
    let lang = lang();
    let fx = fixture();
    let spec = fx.spec.to_string_lossy().into_owned();
    let original = std::fs::read_to_string(&fx.source).unwrap();

    // plan: reports the pending rewrite, mutates nothing.
    assert_eq!(codemod_command(&lang, &argv(&["plan", &spec])), 0);
    assert_eq!(std::fs::read_to_string(&fx.source).unwrap(), original);

    // --check is the CI gate: pending changes exit 1.
    assert_eq!(
        codemod_command(&lang, &argv(&["plan", &spec, "--check"])),
        1
    );

    // apply: writes the rewritten source + kernel bookkeeping.
    assert_eq!(codemod_command(&lang, &argv(&["apply", &spec])), 0);
    let rewritten = std::fs::read_to_string(&fx.source).unwrap();
    assert!(!rewritten.contains(":="), "assigns deleted: {rewritten:?}");
    assert!(
        fx.root
            .join(".handl/codegen/manifest/del-assign.json")
            .exists()
    );
    assert!(fx.root.join(".handl/codegen/journal").is_dir());

    // A second apply is a no-op (already converged).
    assert_eq!(codemod_command(&lang, &argv(&["apply", &spec])), 0);

    // revert: the journal restores the before-image — the only undo.
    assert_eq!(codemod_command(&lang, &argv(&["revert", &spec])), 0);
    assert_eq!(std::fs::read_to_string(&fx.source).unwrap(), original);
}

#[test]
fn revert_with_explicit_run_and_json_plan() {
    let lang = lang();
    let fx = fixture();
    let spec = fx.spec.to_string_lossy().into_owned();

    // --json plan emits parseable JSON carrying the edits/changes.
    assert_eq!(codemod_command(&lang, &argv(&["plan", &spec, "--json"])), 0);

    assert_eq!(codemod_command(&lang, &argv(&["apply", &spec])), 0);
    // `revert --run <bogus>` fails honestly (no such journal).
    assert_eq!(
        codemod_command(&lang, &argv(&["revert", "--run", "no-such-run"])),
        1
    );
}

#[test]
fn missing_spec_is_usage_error_and_bad_spec_is_failure() {
    let lang = lang();
    let fx = fixture();
    let missing = fx.root.join("nope.yaml").to_string_lossy().into_owned();
    assert_eq!(codemod_command(&lang, &argv(&["plan", &missing])), 2);

    let bad = fx.root.join("bad.yaml");
    std::fs::write(&bad, "not: [valid\n").unwrap();
    assert_eq!(
        codemod_command(&lang, &argv(&["plan", &bad.to_string_lossy()])),
        1
    );
}
