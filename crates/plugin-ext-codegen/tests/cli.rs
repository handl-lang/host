//! Smoke tests for the `hnl codegen` argv surface — usage/exit-code
//! contract only (the lifecycle itself is covered by
//! `handl-plugin-codegen`'s registry tests and the kernel suite).

use handl_language::Language;
use handl_plugin_ext_codegen::codegen_command;

fn run(args: &[&str]) -> i32 {
    let lang = Language::default();
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    codegen_command(&lang, &argv)
}

#[test]
fn empty_args_is_usage() {
    assert_eq!(run(&[]), 2);
}

#[test]
fn unknown_target_is_usage() {
    assert_eq!(run(&["bogus", "spec.h", "--output", "/tmp/out"]), 2);
}

#[test]
fn missing_output_is_usage() {
    assert_eq!(run(&["c", "spec.h"]), 2);
}

#[test]
fn spec_and_from_url_are_mutually_exclusive() {
    assert_eq!(
        run(&[
            "c",
            "spec.h",
            "--from-url",
            "https://x/y.h",
            "--output",
            "/tmp/out"
        ]),
        2
    );
}

#[test]
fn destroy_rejects_spec_and_run_flag_on_apply() {
    // `--run` only belongs to revert.
    assert_eq!(
        run(&["c", "spec.h", "--output", "/tmp/o", "--run", "r1"]),
        2
    );
    // destroy takes no spec positional.
    assert_eq!(run(&["destroy", "c", "spec.h", "--output", "/tmp/o"]), 2);
}

#[test]
fn plan_flag_rejects_store_verbs() {
    // `--check`/`--dry-run`/`--plan` only modify apply — pairing one
    // with `destroy`/`revert` is a usage error.
    assert_eq!(run(&["destroy", "c", "--output", "/tmp/o", "--check"]), 2);
    assert_eq!(run(&["revert", "c", "--output", "/tmp/o", "--dry-run"]), 2);
}

#[test]
fn fmt_target_is_registered_and_plans() {
    // PL-004 T-090: the `fmt` target (hnl-tool-fmt's `register`) is
    // linked by default — `plan fmt` on a missing spec reaches the
    // pipeline and fails as `Failed` (exit 1), not `Usage` (exit 2).
    assert_eq!(
        run(&["plan", "fmt", "/nonexistent/file.hnl", "--output", "/tmp/o"]),
        1
    );
}

#[test]
fn plan_verb_reaches_the_pipeline() {
    // A missing spec file fails inside materialize (exit 1) — proving
    // `plan` routed through `CodegenRegistry::plan`, not usage (2).
    assert_eq!(
        run(&["plan", "c", "/nonexistent/spec.h", "--output", "/tmp/o"]),
        1
    );
}

// ---------------------------------------------------------------------------
// `--manifest` mode (PL-004 T-101): mlopt change-set manifest → Engine
// ---------------------------------------------------------------------------

use handl_plugin_codegen::ContentHash;

/// Stage a manifest dir the way `mlopt.manifest.write_manifest` does:
/// `manifest.json` + `blobs/<blake3-hex>` payloads.
fn stage_manifest(dir: &std::path::Path, changes: &[(&str, &[u8])]) {
    std::fs::create_dir_all(dir.join("blobs")).unwrap();
    let wires: Vec<serde_json::Value> = changes
        .iter()
        .map(|(artifact, bytes)| {
            let hex = ContentHash::of(bytes).to_hex();
            std::fs::write(dir.join("blobs").join(&hex), bytes).unwrap();
            serde_json::json!({
                "op": "put",
                "artifact": artifact,
                "before_hash": null,
                "blob_ref": format!("blobs/{hex}"),
                "hash": hex,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "v": 1,
        "hash_algorithm": "blake3-256",
        "target": "cli-smoke",
        "generator_version": "test/v1",
        "source_hashes": {},
        "artifacts": [],
        "run_id": "cli-test",
        "changes": wires,
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .unwrap();
}

#[test]
fn manifest_mode_usage_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let mdir = tmp.path().join("m");
    std::fs::create_dir_all(&mdir).unwrap();
    let m = mdir.to_str().unwrap();
    // Positionals are rejected.
    assert_eq!(run(&["apply", "--manifest", m, "bogus"]), 2);
    // destroy/revert are target-only.
    assert_eq!(run(&["destroy", "--manifest", m]), 2);
    assert_eq!(run(&["revert", "--manifest", m]), 2);
    // Spec-side flags are rejected.
    assert_eq!(run(&["apply", "--manifest", m, "--target", "x"]), 2);
    assert_eq!(run(&["apply", "--manifest", m, "--run", "r1"]), 2);
}

#[test]
fn manifest_apply_and_plan_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let mdir = tmp.path().join("m");
    let root = tmp.path().join("root");
    std::fs::create_dir_all(&mdir).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    stage_manifest(&mdir, &[("out.hnl", b"hello\n")]);
    let m = mdir.to_str().unwrap();
    let r = root.to_str().unwrap();

    // plan --check on a pending create exits 1 (CI gate semantics).
    assert_eq!(run(&["plan", "--manifest", m, "--output", r, "--check"]), 1);
    // plan without --check is informational.
    assert_eq!(run(&["plan", "--manifest", m, "--output", r]), 0);
    // Nothing mutated yet.
    assert!(!root.join("out.hnl").exists());

    // apply writes through Engine::apply.
    assert_eq!(run(&["apply", "--manifest", m, "--output", r]), 0);
    assert_eq!(std::fs::read(root.join("out.hnl")).unwrap(), b"hello\n");

    // Converged: --check now passes.
    assert_eq!(run(&["plan", "--manifest", m, "--output", r, "--check"]), 0);
}

#[test]
fn manifest_missing_dir_is_failure_not_usage() {
    // A nonexistent manifest dir reaches the bridge and fails (1),
    // not the argv parser (2).
    assert_eq!(
        run(&["apply", "--manifest", "/nonexistent/manifest-dir"]),
        1
    );
}
