//! Integration tests for the `hnl run` task runner — real subprocesses
//! (`echo`/`cp`/`cat`/`ls`/`pwd` via PATH), real filesystem, real cache.

use std::path::{Path, PathBuf};

use handl_run::{
    CacheMode, PkgScripts, PlanVerdict, RunError, RunOptions, Runner, TaskStatus, list_scripts,
};

/// A unique scratch package dir per test.
fn pkg_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "hnl-run-it-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_pkg(dir: &Path, manifest: &str) -> PkgScripts {
    std::fs::write(dir.join("pkg.hnl"), manifest).unwrap();
    PkgScripts::load(dir).expect("pkg.hnl loads")
}

fn runner() -> Runner {
    Runner::with_options(RunOptions {
        toolchain: handl_run::Toolchain {
            hnl: "test-hnl".into(),
            ext_eval: "test-eval".into(),
        },
        ..RunOptions::default()
    })
}

fn cache_entries(dir: &Path) -> Vec<PathBuf> {
    let root = dir.join(".handl/cache/run");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            if e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.') {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

fn miss_reason(report: &handl_run::RunReport) -> String {
    report
        .tasks
        .iter()
        .find_map(|t| t.miss_reason.clone())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------
// Execution basics
// ---------------------------------------------------------------------

#[test]
fn string_script_runs_and_is_never_cached() {
    let dir = pkg_dir("str");
    let pkg = write_pkg(&dir, r#"{ scripts: { hi: "echo hello" } }"#);
    let r = runner();

    let rep1 = r.run(&pkg, "hi", &[]).unwrap();
    assert_eq!(rep1.exit_code, 0);
    assert_eq!(rep1.stdout, b"hello\n");
    assert!(matches!(rep1.tasks[0].status, TaskStatus::Ran { code: 0 }));

    let rep2 = r.run(&pkg, "hi", &[]).unwrap();
    assert!(matches!(rep2.tasks[0].status, TaskStatus::Ran { .. }));
    assert!(cache_entries(&dir).is_empty(), "scripts never cache");
    assert!(!dir.join(".handl/cache/run").exists());
}

#[test]
fn array_script_and_and_sequencing() {
    let dir = pkg_dir("arr");
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: { seq: ["echo one && echo two", "echo three"] } }"#,
    );
    let rep = runner().run(&pkg, "seq", &[]).unwrap();
    assert_eq!(rep.exit_code, 0);
    assert_eq!(rep.stdout, b"one\ntwo\nthree\n");
}

#[test]
fn cli_args_append_to_last_command() {
    let dir = pkg_dir("args");
    let pkg = write_pkg(&dir, r#"{ scripts: { t: ["echo first", "echo last"] } }"#);
    let rep = runner().run(&pkg, "t", &["APPENDED".to_string()]).unwrap();
    assert_eq!(rep.stdout, b"first\nlast APPENDED\n");
}

#[test]
fn unknown_script_lists_available_names() {
    let dir = pkg_dir("unknown");
    let pkg = write_pkg(&dir, r#"{ scripts: { alpha: "echo a", beta: "echo b" } }"#);
    let err = runner().run(&pkg, "gamma", &[]).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("gamma"), "{msg}");
    assert!(msg.contains("alpha") && msg.contains("beta"), "{msg}");
}

#[test]
fn metacharacter_command_fails_with_diagnostic() {
    let dir = pkg_dir("meta");
    let pkg = write_pkg(&dir, r#"{ scripts: { bad: "echo hi | cat" } }"#);
    let err = runner().run(&pkg, "bad", &[]).unwrap_err();
    assert!(matches!(err, RunError::Metachar { .. }), "{err}");
    assert!(err.to_string().contains("bash x.sh"), "{err}");
}

#[test]
fn task_failure_propagates_and_skips_dependents() {
    let dir = pkg_dir("fail");
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            bad: { run: "ls /definitely-not-here-xyz", cache: false },
            after: "echo should-not-run",
            top: { run: "echo top", dependsOn: ["bad", "after"] }
        } }"#,
    );
    let rep = runner().run(&pkg, "top", &[]).unwrap();
    assert_ne!(rep.exit_code, 0);
    let statuses: Vec<_> = rep.tasks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(statuses, vec!["bad", "after", "top"]);
    assert!(matches!(rep.tasks[0].status, TaskStatus::Ran { .. }));
    assert!(matches!(rep.tasks[1].status, TaskStatus::Skipped));
    assert!(matches!(rep.tasks[2].status, TaskStatus::Skipped));
    let needle = b"should-not-run";
    assert!(!rep.stdout.windows(needle.len()).any(|w| w == needle));
}

#[test]
fn cwd_runs_relative_to_package_root() {
    let dir = pkg_dir("cwd");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: { where: { run: "pwd", cwd: "sub", cache: false } } }"#,
    );
    let rep = runner().run(&pkg, "where", &[]).unwrap();
    assert_eq!(rep.exit_code, 0);
    let out = String::from_utf8_lossy(&rep.stdout).replace('\\', "/");
    assert!(out.trim_end().ends_with("/sub"), "{out}");
}

#[test]
fn bad_cwd_is_a_diagnostic() {
    let dir = pkg_dir("badcwd");
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: { x: { run: "echo hi", cwd: "nope", cache: false } } }"#,
    );
    let err = runner().run(&pkg, "x", &[]).unwrap_err();
    assert!(matches!(err, RunError::BadCwd { .. }), "{err}");
}

#[test]
fn depth_cap_refuses_nested_runs() {
    let dir = pkg_dir("depth");
    let pkg = write_pkg(&dir, r#"{ scripts: { s: "echo hi" } }"#);
    let r = Runner::with_options(RunOptions {
        depth: Some(handl_run::MAX_RUN_DEPTH),
        ..RunOptions::default()
    });
    let err = r.run(&pkg, "s", &[]).unwrap_err();
    assert!(matches!(err, RunError::DepthExceeded { .. }), "{err}");
    // One below the cap still runs.
    let r2 = Runner::with_options(RunOptions {
        depth: Some(handl_run::MAX_RUN_DEPTH - 1),
        ..RunOptions::default()
    });
    assert_eq!(r2.run(&pkg, "s", &[]).unwrap().exit_code, 0);
}

// ---------------------------------------------------------------------
// Scheduling
// ---------------------------------------------------------------------

#[test]
fn depends_on_runs_deps_first_and_restores_before_dependents() {
    let dir = pkg_dir("dag");
    std::fs::write(dir.join("seed.txt"), "dep-data\n").unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            gen: { run: ["cp seed.txt dep.txt", "echo generated"], outputs: ["dep.txt"], inputs: ["seed.txt"] },
            use: { run: "cat dep.txt", inputs: ["dep.txt"], dependsOn: ["gen"] }
        } }"#,
    );
    let r = runner();

    let rep1 = r.run(&pkg, "use", &[]).unwrap();
    assert_eq!(rep1.order, vec!["gen", "use"]);
    assert_eq!(rep1.exit_code, 0);
    assert!(rep1.stdout.windows(8).any(|w| w == b"dep-data"));

    // Delete the dep's output: on rerun `gen` hits and restores it
    // *before* `use`'s inputs are fingerprinted — `use` still hits.
    std::fs::remove_file(dir.join("dep.txt")).unwrap();
    let rep2 = r.run(&pkg, "use", &[]).unwrap();
    assert!(dir.join("dep.txt").is_file(), "dep output restored");
    let use_result = rep2.tasks.iter().find(|t| t.name == "use").unwrap();
    assert!(
        matches!(use_result.status, TaskStatus::CacheHit { .. }),
        "{:?}",
        rep2.tasks
    );
    assert!(rep2.stdout.windows(8).any(|w| w == b"dep-data"));
}

// ---------------------------------------------------------------------
// Cache: hit / miss / drift / busts
// ---------------------------------------------------------------------

#[test]
fn cache_hit_replays_streams_and_restores_outputs() {
    let dir = pkg_dir("hit");
    std::fs::write(dir.join("seed.txt"), "v1\n").unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            gen: { run: ["cp seed.txt out.txt", "echo made-it"], inputs: ["seed.txt"], outputs: ["out.txt"] }
        } }"#,
    );
    let r = runner();

    let rep1 = r.run(&pkg, "gen", &[]).unwrap();
    assert_eq!(rep1.exit_code, 0);
    assert!(matches!(rep1.tasks[0].status, TaskStatus::Ran { code: 0 }));
    assert_eq!(
        std::fs::read_to_string(dir.join("out.txt")).unwrap(),
        "v1\n"
    );
    assert_eq!(cache_entries(&dir).len(), 1);
    let entry = &cache_entries(&dir)[0];
    assert!(entry.join("meta.json").is_file());
    assert!(entry.join("stdout").is_file());
    assert!(entry.join("stderr").is_file());
    assert!(entry.join("outputs/out.txt").is_file());

    // Delete outputs → hit restores them and replays streams verbatim.
    std::fs::remove_file(dir.join("out.txt")).unwrap();
    let rep2 = r.run(&pkg, "gen", &[]).unwrap();
    assert!(matches!(rep2.tasks[0].status, TaskStatus::CacheHit { .. }));
    assert_eq!(
        std::fs::read_to_string(dir.join("out.txt")).unwrap(),
        "v1\n"
    );
    assert_eq!(rep2.stdout, b"made-it\n", "streams replayed verbatim");
    assert!(rep2.messages.iter().any(|m| m.contains("cache hit")));
}

#[test]
fn output_drift_is_a_miss_not_a_clobber() {
    let dir = pkg_dir("drift");
    std::fs::write(dir.join("seed.txt"), "v1\n").unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            gen: { run: ["cp seed.txt out.txt"], inputs: ["seed.txt"], outputs: ["out.txt"] }
        } }"#,
    );
    let r = runner();
    r.run(&pkg, "gen", &[]).unwrap();

    // User edits the output → drift → miss, re-executes (cp restores
    // the declared content), re-caches.
    std::fs::write(dir.join("out.txt"), "user-work\n").unwrap();
    let rep = r.run(&pkg, "gen", &[]).unwrap();
    assert!(matches!(rep.tasks[0].status, TaskStatus::Ran { .. }));
    assert!(
        miss_reason(&rep).contains("drifted"),
        "{}",
        miss_reason(&rep)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("out.txt")).unwrap(),
        "v1\n"
    );
}

#[test]
fn input_modified_names_the_culprit() {
    let dir = pkg_dir("inmod");
    std::fs::write(dir.join("seed.txt"), "v1\n").unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            gen: { run: ["cp seed.txt out.txt"], inputs: ["seed.txt"], outputs: ["out.txt"] }
        } }"#,
    );
    let r = runner();
    r.run(&pkg, "gen", &[]).unwrap();

    std::fs::write(dir.join("seed.txt"), "v2\n").unwrap();
    let rep = r.run(&pkg, "gen", &[]).unwrap();
    assert!(
        miss_reason(&rep).contains("seed.txt"),
        "{}",
        miss_reason(&rep)
    );
    assert!(
        miss_reason(&rep).contains("modified"),
        "{}",
        miss_reason(&rep)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("out.txt")).unwrap(),
        "v2\n"
    );

    // And it re-cached: next run hits again.
    let rep2 = r.run(&pkg, "gen", &[]).unwrap();
    assert!(matches!(rep2.tasks[0].status, TaskStatus::CacheHit { .. }));
}

#[test]
fn input_added_and_removed_are_misses() {
    let dir = pkg_dir("inadd");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/a.hnl"), "a").unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            t: { run: "echo ok", inputs: ["src/**/*"], cache: true }
        } }"#,
    );
    let r = runner();
    r.run(&pkg, "t", &[]).unwrap();

    std::fs::write(dir.join("src/b.hnl"), "b").unwrap();
    let rep = r.run(&pkg, "t", &[]).unwrap();
    assert!(miss_reason(&rep).contains("added"), "{}", miss_reason(&rep));
    assert!(miss_reason(&rep).contains("b.hnl"), "{}", miss_reason(&rep));

    std::fs::remove_file(dir.join("src/b.hnl")).unwrap();
    let rep2 = r.run(&pkg, "t", &[]).unwrap();
    // b.hnl was recorded by the previous (post-add) run → now removed.
    assert!(
        miss_reason(&rep2).contains("removed"),
        "{}",
        miss_reason(&rep2)
    );
}

#[test]
fn default_inputs_cover_pkg_hnl_and_module_sources() {
    let dir = pkg_dir("defin");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/m.hnl"), "export x := 1\n").unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ modules: ["m"], scripts: { t: { run: "echo ok" } } }"#,
    );
    let r = runner();
    r.run(&pkg, "t", &[]).unwrap();
    let rep = r.run(&pkg, "t", &[]).unwrap();
    assert!(matches!(rep.tasks[0].status, TaskStatus::CacheHit { .. }));

    // Touching pkg.hnl itself busts the default input set.
    std::fs::write(
        dir.join("pkg.hnl"),
        r#"{ modules: ["m"], scripts: { t: { run: "echo ok", description: "x" } } }"#,
    )
    .unwrap();
    // description isn't a script key — def unchanged, but pkg.hnl
    // content changed → input 'pkg.hnl' modified.
    let pkg2 = PkgScripts::load(&dir).unwrap();
    let rep2 = r.run(&pkg2, "t", &[]).unwrap();
    assert!(
        miss_reason(&rep2).contains("pkg.hnl"),
        "{}",
        miss_reason(&rep2)
    );
}

#[test]
fn undeclared_reads_hit_stale_documented_gap() {
    // P-003 known v1 gap, pinned: a command reading a file that is not
    // in `inputs:` (and not a module source/pkg.hnl) demonstrably hits
    // stale. `bash ci.sh` is the canonical case; `cat` stands in.
    let dir = pkg_dir("stale");
    std::fs::write(dir.join("seed.txt"), "v1\n").unwrap();
    let pkg = write_pkg(&dir, r#"{ scripts: { t: { run: "cat seed.txt" } } }"#);
    let r = runner();
    let rep1 = r.run(&pkg, "t", &[]).unwrap();
    assert_eq!(rep1.stdout, b"v1\n");

    std::fs::write(dir.join("seed.txt"), "v2\n").unwrap();
    let rep2 = r.run(&pkg, "t", &[]).unwrap();
    assert!(matches!(rep2.tasks[0].status, TaskStatus::CacheHit { .. }));
    assert_eq!(rep2.stdout, b"v1\n", "stale replay — the documented gap");
}

#[test]
fn env_and_toolchain_and_args_each_bust() {
    let dir = pkg_dir("busts");
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            t: { run: "echo ok", env: ["T77_X"], inputs: [] }
        } }"#,
    );
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var("T77_X") };
    let r = runner();
    r.run(&pkg, "t", &[]).unwrap();
    let rep = r.run(&pkg, "t", &[]).unwrap();
    assert!(matches!(rep.tasks[0].status, TaskStatus::CacheHit { .. }));

    // env change busts.
    unsafe { std::env::set_var("T77_X", "1") };
    let rep = r.run(&pkg, "t", &[]).unwrap();
    assert!(
        miss_reason(&rep).contains("env 'T77_X' changed"),
        "{}",
        miss_reason(&rep)
    );

    // toolchain change busts (fresh entry under same key? no —
    // toolchain is compared via meta, same dir).
    let r2 = Runner::with_options(RunOptions {
        toolchain: handl_run::Toolchain {
            hnl: "other".into(),
            ext_eval: "test-eval".into(),
        },
        ..RunOptions::default()
    });
    let rep = r2.run(&pkg, "t", &[]).unwrap();
    assert!(
        miss_reason(&rep).contains("toolchain changed"),
        "{}",
        miss_reason(&rep)
    );

    // args change → different key → no cache entry.
    unsafe { std::env::remove_var("T77_X") };
    let rep = r.run(&pkg, "t", &["a".to_string()]).unwrap();
    assert!(
        miss_reason(&rep).contains("no cache entry"),
        "{}",
        miss_reason(&rep)
    );
    let rep = r.run(&pkg, "t", &["a".to_string()]).unwrap();
    assert!(matches!(rep.tasks[0].status, TaskStatus::CacheHit { .. }));
}

#[test]
fn failure_is_never_cached() {
    let dir = pkg_dir("nocachefail");
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: { bad: { run: "ls /definitely-not-here-xyz", inputs: [] } } }"#,
    );
    let r = runner();
    let rep = r.run(&pkg, "bad", &[]).unwrap();
    assert_ne!(rep.exit_code, 0);
    assert!(cache_entries(&dir).is_empty(), "failures never cache");
    // And it keeps failing (not a stale hit).
    let rep2 = r.run(&pkg, "bad", &[]).unwrap();
    assert_ne!(rep2.exit_code, 0);
}

#[test]
fn cache_false_and_no_cache_mode_bypass() {
    let dir = pkg_dir("optout");
    std::fs::write(dir.join("seed.txt"), "v1\n").unwrap();
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            noc: { run: "echo nc", cache: false, inputs: ["seed.txt"] },
            yes: { run: "echo yes", inputs: ["seed.txt"] }
        } }"#,
    );
    let r = runner();
    r.run(&pkg, "noc", &[]).unwrap();
    r.run(&pkg, "noc", &[]).unwrap();
    assert!(cache_entries(&dir).is_empty(), "cache:false never stores");

    // --no-cache: lookup *and* store bypassed.
    let r_nc = Runner::with_options(RunOptions {
        cache: CacheMode::Disabled,
        toolchain: handl_run::Toolchain {
            hnl: "test-hnl".into(),
            ext_eval: "test-eval".into(),
        },
        ..RunOptions::default()
    });
    let rep = r_nc.run(&pkg, "yes", &[]).unwrap();
    assert!(matches!(rep.tasks[0].status, TaskStatus::Ran { .. }));
    assert!(cache_entries(&dir).is_empty(), "--no-cache never stores");
    // A second --no-cache run still runs (never hits).
    let rep2 = r_nc.run(&pkg, "yes", &[]).unwrap();
    assert!(matches!(rep2.tasks[0].status, TaskStatus::Ran { .. }));
}

#[test]
fn plan_reports_verdicts_without_executing() {
    let dir = pkg_dir("plan");
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: {
            gen: { run: "echo g", inputs: [] },
            top: { run: "echo t", dependsOn: ["gen"] },
            loose: "echo l"
        } }"#,
    );
    let r = runner();
    let plan = r.plan(&pkg, "top", &[]).unwrap();
    assert_eq!(plan.order, vec!["gen", "top"]);
    assert!(matches!(plan.tasks[0].verdict, PlanVerdict::Miss(_)));
    assert!(matches!(plan.tasks[1].verdict, PlanVerdict::Miss(_)));

    // Run once, then plan again: cached tasks hit, script uncached.
    r.run(&pkg, "top", &[]).unwrap();
    let plan2 = r.plan(&pkg, "top", &[]).unwrap();
    assert!(matches!(plan2.tasks[0].verdict, PlanVerdict::Hit));
    assert!(matches!(plan2.tasks[1].verdict, PlanVerdict::Hit));

    let plan3 = r.plan(&pkg, "loose", &[]).unwrap();
    assert!(matches!(plan3.tasks[0].verdict, PlanVerdict::Uncached));
    // Plan did not execute: no stray stdout was produced by it.
    assert!(plan3.tasks.len() == 1);
}

#[test]
fn list_scripts_gives_usage_lines() {
    let dir = pkg_dir("list");
    let pkg = write_pkg(
        &dir,
        r#"{ scripts: { zebra: "echo z", apple: "echo a && echo b" } }"#,
    );
    let lines = list_scripts(&pkg);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("apple"), "{lines:?}");
    assert!(lines[1].starts_with("zebra"), "{lines:?}");
}

#[test]
fn discover_finds_nearest_pkg_hnl() {
    let dir = pkg_dir("disc");
    let nested = dir.join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(dir.join("pkg.hnl"), r#"{ scripts: { s: "echo hi" } }"#).unwrap();
    let pkg = PkgScripts::discover(&nested).expect("discovers pkg.hnl");
    assert_eq!(pkg.dir, std::fs::canonicalize(&dir).unwrap());
    assert!(pkg.get("s").is_some());
}

/// Serializes env-mutating tests (edition 2024 `set_var` is unsafe).
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
