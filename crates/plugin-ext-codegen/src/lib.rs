//! `hnl codegen` CLI extension (PL-004 T-076) — the desired-state
//! codegen command surface over the `hnl-engine-codegen` kernel.
//!
//! This crate is a lookup over the [`CodegenRegistry`]: each linked
//! target module (`c`, `webidl`, `python`, `cli`, `fmt`, …) registers its own
//! backend, emitter, doc pass, and spec input — there is no per-target
//! `match` here. Hosts opt targets in through this crate's Cargo
//! features (`c`, `webidl`, `python`, `cli`, `fmt`; all on by default), each
//! feature enabling one repo module's dependency (ADR-0128 §"Registration:
//! link-time first").
//!
//! # Command surface
//!
//! ```text
//! hnl codegen [plan|apply|destroy|revert] <target> [<spec> | --from-url <url>] --output <dir> [flags]
//! ```
//!
//! The lifecycle verb defaults to `apply` (the pre-PL-004 `sdkgen`
//! behaviour): `plan` materializes the desired [`Change`] set and diffs
//! it against the store — no mutation; `--check`/`--dry-run` are
//! `plan` aliases (`--check` exits 1 when the plan has pending
//! mutations, for CI gates). `destroy` deletes exactly the artifacts
//! the target's prior manifest records; `revert [--run <id>]` undoes a
//! journal run (default: the run that produced the current manifest).

use handl_language::Language;
use handl_plugin_codegen::{
    ApplyReport, CodegenRegistry, CodegenRequest, CodegenTargetError, Plan, PlanAction,
    RevertReport, SpecModule, http_get,
};
use std::path::{Path, PathBuf};

/// The kernel lifecycle verb `hnl codegen` dispatches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verb {
    /// Reconcile the store to the desired state (default).
    Apply,
    /// Diff desired vs. store, no mutation (`--check`/`--dry-run`).
    Plan,
    /// Delete the artifacts the target's manifest records.
    Destroy,
    /// Undo a journal run's before-images.
    Revert,
}

impl Verb {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "apply" => Some(Self::Apply),
            "plan" => Some(Self::Plan),
            "destroy" => Some(Self::Destroy),
            "revert" => Some(Self::Revert),
            _ => None,
        }
    }

    fn needs_spec(self) -> bool {
        matches!(self, Self::Apply | Self::Plan)
    }
}

/// The registry populated with every target this build linked in.
///
/// Modules register by name — the `python` target record composes
/// backend `c` + emitter `python` without `python/` crate-depending on
/// `c/`'s module. A host may extend the returned registry with
/// additional modules before driving it itself.
pub fn linked_targets() -> CodegenRegistry {
    // `mut` is exercised only when at least one target feature is on.
    #[allow(unused_mut)]
    let mut registry = CodegenRegistry::new();
    #[cfg(any(feature = "c", feature = "webidl"))]
    handl_plugin_codegen_emitter_file::register(&mut registry);
    #[cfg(feature = "c")]
    handl_plugin_codegen_backend_c::register(&mut registry);
    #[cfg(feature = "webidl")]
    handl_plugin_codegen_backend_webidl::register(&mut registry);
    #[cfg(feature = "python")]
    handl_plugin_codegen_emitter_python::register(&mut registry);
    #[cfg(feature = "cli")]
    handl_plugin_codegen_cli::register(&mut registry);
    // PL-004 T-090: the `fmt` target — `FmtSource`/`FmtEmitter` over the
    // token-stream `FmtModule` IR. `hnl codegen fmt <file.hnl> --output
    // <dir>` formats file→file through the same plan/apply lifecycle:
    // `--check`/`plan` exits 1 on drift, `apply` writes via
    // `Engine::apply` (journaled under `.handl/codegen/`).
    #[cfg(feature = "fmt")]
    hnl_engine_fmt::register(&mut registry);
    registry
}

/// Run the `hnl codegen` subcommand and return the process exit code.
///
/// `lang` is the language the emitted `.hnl` port/adapter sources are
/// canonicalized against — injected by the host (ADR-0128 §"The 0.8
/// seam") because this crate must not link `handl-kit-data-prelude`.
pub fn codegen_command(lang: &Language, args: &[String]) -> i32 {
    let registry = linked_targets();
    let usage = format!(
        "usage: hnl codegen [plan|apply|destroy|revert] <{}> [<spec> | --from-url <url>] --output <dir> [--target <triple>] [--include <glob>] [--exclude <glob>] [--config <yaml>] [--source-url <url>] [--emit-spec <path|->] [--cache-dir <dir>] [--check|--dry-run] [--run <id>]",
        registry.target_names().join("|")
    );
    if args.is_empty() {
        eprintln!("{usage}");
        return 2;
    }

    // Leading verb, if present; otherwise `apply` (the `sdkgen`
    // default) unless a plan flag appears below.
    let (mut verb, rest) = match Verb::parse(&args[0]) {
        Some(v) => (v, &args[1..]),
        None => (Verb::Apply, args),
    };
    if rest.is_empty() {
        eprintln!("{usage}");
        return 2;
    }

    let mut positionals: Vec<String> = Vec::new();
    let mut output: Option<String> = None;
    let mut target: Option<String> = None;
    let mut include: Option<String> = None;
    let mut exclude: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut from_url: Option<String> = None;
    let mut source_url_flag: Option<String> = None;
    let mut emit_spec: Option<String> = None;
    let mut cache_dir: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut check = false;

    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--output" => {
                i += 1;
                output = rest.get(i).map(String::to_string);
            }
            "--target" => {
                i += 1;
                target = rest.get(i).map(String::to_string);
            }
            "--include" => {
                i += 1;
                include = rest.get(i).map(String::to_string);
            }
            "--exclude" => {
                i += 1;
                exclude = rest.get(i).map(String::to_string);
            }
            "--config" => {
                i += 1;
                config_path = rest.get(i).map(String::to_string);
            }
            "--from-url" => {
                i += 1;
                from_url = rest.get(i).map(String::to_string);
            }
            "--source-url" => {
                i += 1;
                // Provenance only: stamp `--- source:` without fetching.
                source_url_flag = rest.get(i).map(String::to_string);
            }
            "--emit-spec" => {
                i += 1;
                // Backend hint (`cli` target): dump the resolved spec —
                // `-` writes it to stdout.
                emit_spec = rest.get(i).map(String::to_string);
            }
            "--cache-dir" => {
                i += 1;
                // Doc/spec cache root (e.g. webidl's MDN cache) —
                // deliberately outside the output tree.
                cache_dir = rest.get(i).map(String::to_string);
            }
            "--run" => {
                i += 1;
                run_id = rest.get(i).map(String::to_string);
            }
            "--check" | "--dry-run" | "--plan" => {
                if rest[i] == "--check" {
                    check = true;
                }
                if verb == Verb::Apply {
                    verb = Verb::Plan;
                } else if verb != Verb::Plan {
                    eprintln!("codegen: --check/--dry-run/--plan are only valid for apply/plan");
                    return 2;
                }
            }
            s if s.starts_with("--") => {
                eprintln!("codegen: unknown flag `{s}`");
                return 2;
            }
            s => positionals.push(s.to_string()),
        }
        i += 1;
    }

    // Positionals: `<target>` always; `<spec>` only for apply/plan —
    // and there `--from-url` is an alternative to the positional, so
    // arity is 1..=2. destroy/revert take the target alone.
    let arity_ok = if verb.needs_spec() {
        (1..=2).contains(&positionals.len())
    } else {
        positionals.len() == 1
    };
    if !arity_ok {
        eprintln!("{usage}");
        return 2;
    }
    let target_name = positionals[0].clone();
    let spec_arg = positionals.get(1).cloned();

    if !registry.contains(&target_name) {
        eprintln!("{usage}");
        return 2;
    }

    let Some(output) = output else {
        eprintln!("codegen: missing --output");
        return 2;
    };
    if run_id.is_some() && verb != Verb::Revert {
        eprintln!("codegen: --run is only valid for `revert`");
        return 2;
    }
    if verb.needs_spec() {
        if spec_arg.is_some() && from_url.is_some() {
            eprintln!("codegen: <spec> and --from-url are mutually exclusive");
            return 2;
        }
        if from_url.is_some() && source_url_flag.is_some() {
            eprintln!("codegen: --from-url already records the fetched URL as the source");
            return 2;
        }
    } else {
        // destroy/revert are store-side operations: no spec input, and
        // --run belongs to revert only.
        if spec_arg.is_some() || from_url.is_some() || source_url_flag.is_some() {
            eprintln!(
                "codegen: {} takes no <spec>/--from-url/--source-url",
                verb_name(verb)
            );
            return 2;
        }
        // --run belongs to revert only (checked for every verb below).
        if target.is_some()
            || include.is_some()
            || exclude.is_some()
            || config_path.is_some()
            || emit_spec.is_some()
            || cache_dir.is_some()
        {
            eprintln!(
                "codegen: {} takes no --target/--include/--exclude/--config/--emit-spec/--cache-dir",
                verb_name(verb)
            );
            return 2;
        }
    }

    let output_path = Path::new(&output);

    // Resolve the input spec for apply/plan: either a local path or a
    // downloaded URL. The fetched file lands next to the output dir (the
    // `generated/` spec convention) so the port's `module.library` still
    // derives from the upstream basename. `source_url` is kept to stamp
    // the module's provenance (emitted as `--- source:` header lines).
    let source_url = from_url.clone().or(source_url_flag);
    // Holds a `--from-url` fetch so `spec_path` can borrow it for the
    // rest of the command.
    let fetched_spec: Option<PathBuf>;
    let spec_path: &Path = if verb.needs_spec() {
        match (&spec_arg, &from_url) {
            (Some(s), None) => Path::new(s),
            (None, Some(url)) => {
                let spec_dir = output_path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new("."));
                match fetch_spec(url, spec_dir) {
                    Ok(p) => {
                        eprintln!("codegen: fetched {url} -> {}", p.display());
                        fetched_spec = Some(p);
                        fetched_spec.as_deref().unwrap()
                    }
                    Err(e) => {
                        eprintln!("codegen: {e}");
                        return 1;
                    }
                }
            }
            (None, None) => {
                eprintln!("{usage}");
                return 2;
            }
            (Some(_), Some(_)) => unreachable!(),
        }
    } else {
        // destroy/revert never read the spec — the request field is
        // non-optional, so hand it an empty path it will not touch.
        Path::new("")
    };

    // Input-kind routing (T-076): the `python` target's spec-driven
    // variant is a *separate* registry target (`python-spec` over
    // `SpecModule`) because one target pairs exactly one IR family —
    // choosing between them is the command layer's job. A yaml/json
    // spec under `python` routes to `python-spec` when that target is
    // linked; a C header still routes to `python` (the ctypes
    // projection over the `c` source).
    let mut effective_target = target_name.as_str();
    if effective_target == "python"
        && SpecModule::format_for(spec_path).is_some()
        && registry.contains("python-spec")
    {
        effective_target = "python-spec";
    }

    let request = CodegenRequest {
        spec: spec_path,
        source_url: source_url.as_deref(),
        output: output_path,
        target: target.as_deref(),
        include: include.as_deref(),
        exclude: exclude.as_deref(),
        config: config_path.as_deref().map(Path::new),
        emit_spec: emit_spec.as_deref().map(Path::new),
        lang,
        cache_dir: cache_dir.as_deref().map(Path::new),
    };

    match verb {
        Verb::Apply => match registry.run(effective_target, &request) {
            Ok(()) => 0,
            Err(e) => report_error(&e),
        },
        Verb::Plan => match registry.plan(effective_target, &request) {
            Ok(plan) => {
                print_plan(&plan);
                // `--check` is the CI gate: pending mutations are a
                // failure; `--dry-run`/`plan` are informational.
                if check && !plan.is_clean() { 1 } else { 0 }
            }
            Err(e) => report_error(&e),
        },
        Verb::Destroy => match registry.destroy(effective_target, &request) {
            Ok(report) => {
                print_apply_report("destroy", effective_target, &report);
                if report.failures.is_empty() { 0 } else { 1 }
            }
            Err(e) => report_error(&e),
        },
        Verb::Revert => match registry.revert(effective_target, &request, run_id.as_deref()) {
            Ok(report) => {
                print_revert_report(effective_target, &report);
                if report.failures.is_empty() && report.fatal.is_none() {
                    0
                } else {
                    1
                }
            }
            Err(e) => report_error(&e),
        },
    }
}

fn verb_name(v: Verb) -> &'static str {
    match v {
        Verb::Apply => "apply",
        Verb::Plan => "plan",
        Verb::Destroy => "destroy",
        Verb::Revert => "revert",
    }
}

/// Map a [`CodegenTargetError`] to its exit code (2 usage / 1 failed),
/// mirroring the pre-PL-004 `sdkgen` convention.
fn report_error(e: &CodegenTargetError) -> i32 {
    match e {
        CodegenTargetError::Usage(msg) => {
            eprintln!("codegen: {msg}");
            2
        }
        CodegenTargetError::Failed(msg) => {
            eprintln!("codegen: {msg}");
            1
        }
    }
}

/// Print a [`Plan`] diff: counts on the first line, one entry per
/// pending mutation (`+` create / `~` update / `-` delete).
fn print_plan(plan: &Plan) {
    println!(
        "plan: {} create, {} update, {} delete, {} unchanged",
        plan.count(PlanAction::Create),
        plan.count(PlanAction::Update),
        plan.count(PlanAction::Delete),
        plan.count(PlanAction::Unchanged),
    );
    for entry in plan.mutations() {
        let tag = match entry.action {
            PlanAction::Create => "+",
            PlanAction::Update => "~",
            PlanAction::Delete => "-",
            PlanAction::Unchanged => " ",
        };
        println!("  {tag} {}", entry.artifact);
    }
}

fn print_apply_report(verb: &str, target: &str, report: &ApplyReport) {
    println!(
        "{verb} {target}: run {} — {} created, {} updated, {} deleted, {} unchanged, {} skipped, {} failed",
        report.run_id,
        report.created,
        report.updated,
        report.deleted,
        report.unchanged,
        report.skipped,
        report.failures.len(),
    );
    for (artifact, err) in &report.failures {
        eprintln!("codegen: {artifact}: {err}");
    }
}

fn print_revert_report(target: &str, report: &RevertReport) {
    println!(
        "revert {target}: run {} — {} restored, {} removed, {} skipped, {} failed{}",
        report.run_id,
        report.restored,
        report.removed,
        report.skipped,
        report.failures.len(),
        if report.aborted { " (aborted)" } else { "" },
    );
    for (artifact, err) in &report.failures {
        eprintln!("codegen: {artifact}: {err}");
    }
    if let Some(fatal) = &report.fatal {
        eprintln!("codegen: {fatal}");
    }
}

/// Download `url` into `dir` under the URL's basename and return the local
/// path. The basename is preserved because backends derive the module/library
/// name from the input file stem.
fn fetch_spec(url: &str, dir: &Path) -> Result<PathBuf, String> {
    let basename = url
        .split(['?', '#'])
        .next()
        .and_then(|u| u.rsplit('/').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("spec")
        .to_string();

    let body = http_get(url, 30_000).map_err(|e| format!("failed to fetch {url}: {e}"))?;

    std::fs::create_dir_all(dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    let path = dir.join(basename);
    std::fs::write(&path, &body).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(path)
}
