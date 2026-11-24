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

// ---------------------------------------------------------------------------
// `handl:<mod>.<fn>` package-module wiring (P-002/T-108)
// ---------------------------------------------------------------------------

/// Install the canonical default language once per test binary — the
/// `HandlModule` bridge runtimes `handl:` rewrites evaluate through
/// resolve it lazily (the same seam kit assembly uses).
fn ensure_default_language() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(lang());
    }
}

const HANDL_SPEC: &str = r#"name: pkg-del
version: 1
targets:
  - language: handl
    glob: "targets/**/*.hnl"
    unparse: preserve
rules:
  - id: del
    match: "assign"
    rewrite: "handl:t.rewrite"
"#;

/// A package fixture: `pkg.hnl` declaring module `t` at `src/index.hnl`
/// (the ADR-0160 single-module layout), plus `codemods/codemod.yaml`
/// (walk-up discovery target) and `targets/a.hnl` (the rewrite input).
struct PkgFixture {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    spec: std::path::PathBuf,
    source: std::path::PathBuf,
}

fn pkg_fixture() -> PkgFixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("codemods")).unwrap();
    std::fs::create_dir_all(root.join("targets")).unwrap();
    std::fs::write(root.join("pkg.hnl"), "{ modules: [\"t\"] }\n").unwrap();
    // `export`-per-file shape: `discover_funcs` exposes `rewrite`.
    std::fs::write(
        root.join("src/index.hnl"),
        "export rewrite := n ~> ({ kind: \"__codemod_delete__\" })\n",
    )
    .unwrap();
    let spec = root.join("codemods/codemod.yaml");
    std::fs::write(&spec, HANDL_SPEC).unwrap();
    let source = root.join("targets/a.hnl");
    std::fs::write(&source, "A := 1\nB := 2\n").unwrap();
    PkgFixture {
        _dir: dir,
        root,
        spec,
        source,
    }
}

#[test]
fn handl_rewrite_resolves_via_pkg_hnl_walkup() {
    ensure_default_language();
    let lang = lang();
    let fx = pkg_fixture();
    let spec = fx.spec.to_string_lossy().into_owned();
    let root = fx.root.to_string_lossy().into_owned();
    let original = std::fs::read_to_string(&fx.source).unwrap();

    // plan → apply: the spec's `handl:t.rewrite` resolves against the
    // package module discovered by walking up from `codemods/`.
    assert_eq!(
        codemod_command(&lang, &argv(&["plan", &spec, "--root", &root])),
        0
    );
    assert_eq!(
        codemod_command(&lang, &argv(&["apply", &spec, "--root", &root])),
        0
    );
    let rewritten = std::fs::read_to_string(&fx.source).unwrap();
    assert_ne!(rewritten, original, "handl: rewrite must edit the file");
    assert!(!rewritten.contains(":="), "assigns deleted: {rewritten:?}");
}

#[test]
fn handl_rewrite_resolves_via_package_flag() {
    ensure_default_language();
    let lang = lang();
    let pkg = pkg_fixture();
    // A spec OUTSIDE any package: walk-up finds no `pkg.hnl`, so only
    // `--package` can wire the module.
    let dir = tempfile::tempdir().unwrap();
    let loose = dir.path().join("codemod.yaml");
    std::fs::write(&loose, HANDL_SPEC).unwrap();
    let targets = dir.path().join("targets");
    std::fs::create_dir_all(&targets).unwrap();
    let source = targets.join("a.hnl");
    std::fs::write(&source, "A := 1\n").unwrap();

    let spec = loose.to_string_lossy().into_owned();
    let root = dir.path().to_string_lossy().into_owned();
    let pkgdir = pkg.root.to_string_lossy().into_owned();
    let _ = pkg; // keep the tempdir alive for the module source

    assert_eq!(
        codemod_command(
            &lang,
            &argv(&["apply", &spec, "--root", &root, "--package", &pkgdir])
        ),
        0
    );
    assert!(!std::fs::read_to_string(&source).unwrap().contains(":="));
}

#[test]
fn handl_rewrite_without_package_is_honest_noop() {
    ensure_default_language();
    let lang = lang();
    // Same spec shape but no `pkg.hnl` anywhere above it and no
    // `--package`: the `handl:` name must NOT silently pass — the rule
    // fails as a RewriteError diagnostic and nothing is rewritten.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("targets")).unwrap();
    let spec = root.join("codemod.yaml");
    std::fs::write(&spec, HANDL_SPEC).unwrap();
    let source = root.join("targets/a.hnl");
    std::fs::write(&source, "A := 1\n").unwrap();

    let spec_s = spec.to_string_lossy().into_owned();
    assert_eq!(codemod_command(&lang, &argv(&["apply", &spec_s])), 0);
    assert!(
        std::fs::read_to_string(&source).unwrap().contains(":="),
        "unresolved handl: rewrite must leave the file untouched"
    );
}
