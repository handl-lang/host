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
fn plan_verb_reaches_the_pipeline() {
    // A missing spec file fails inside materialize (exit 1) — proving
    // `plan` routed through `CodegenRegistry::plan`, not usage (2).
    assert_eq!(
        run(&["plan", "c", "/nonexistent/spec.h", "--output", "/tmp/o"]),
        1
    );
}
