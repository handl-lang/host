//! `hnl codemod` CLI extension (PL-004 T-086) — the declarative codemod
//! command surface over `hnl-tool-codemod` + the `hnl-tool-codegen`
//! lifecycle kernel.
//!
//! # Command surface
//!
//! ```text
//! hnl codemod [plan|apply|revert] <spec.yaml> [--root <dir>] [--target <glob>]
//!             [--package <dir>] [--json] [--check|--dry-run] [--run <id>]
//!             [--keep-going]
//! ```
//!
//! - `plan` (also `--check`/`--dry-run`) — resolve `targets[].glob`,
//!   run the codemod engine, print edits + the materialized `Change`
//!   set. `--check` is the CI gate: pending changes exit 1.
//! - `apply` (the default verb, matching `hnl codegen`) — `Engine::apply`
//!   over the plan's `Change` set: write-ahead journal + manifest under
//!   `<root>/.handl/codegen/` (the kernel's fixed reserved prefix —
//!   codemod reuses the codegen bookkeeping layout, there is no
//!   `.handl/codemod/`).
//! - `revert [--run <id>]` — `Engine::revert` on the journal: the ONLY
//!   undo path. Codemod **never destroys** — it does not own artifacts,
//!   so there is no `destroy` verb and `apply` passes no prior manifest
//!   (a file that stops matching is left alone, never deleted).
//!
//! `--root` sets both the glob base and the `FsStore` root; default is
//! the spec file's directory (package-relative globs, per P-002's
//! "codemod.yaml in a package"). `revert` may omit the spec when
//! `--run` names the journal run outright.
//!
//! `lang` is the canonical kit language injected by the host (ADR-0128
//! §"The 0.8 seam") — registered as the `"handl"` target language.
//!
//! # Resolver wiring
//!
//! [`codemod_command`] ships the kit-language wiring only; hosts that
//! can supply more call [`codemod_command_with`], which forwards
//! extra [`NativeRewrite`] layers into
//! [`Codemod::with_rewrite_layer`] — the `hnl-ext-codemod` packaging
//! passes the kit's `handl:` `.hnl` rewrite adapter
//! (`Kit::handl_rewrite_adapter`), so `rewrite: "handl:<mod>.<fn>"`
//! rules dispatch into kit modules. On top of that the command wires
//! **package modules** itself (P-002/T-108): when the spec names a
//! `handl:` rewrite, the package owning the spec — `--package <dir>`
//! when given, otherwise the nearest `pkg.hnl` walking up from the
//! spec path — contributes every declared module via
//! [`Codemod::with_package_modules`]. A `handl:` name no module
//! provides stays an honest `RewriteError` diagnostic on the first
//! match. Still unwired by the CLI:
//!
//! - `native:`/bare rewrite functions — there is no canonical codemod
//!   native set; hosts register them via [`Codemod::with_native`] or a
//!   [`CodemodRuleProvider`](handl_plugin_codemod::CodemodRuleProvider)
//!   (the lint `--fix` path is the intended first consumer).
//! - `requires:` [`EngineContextProvider`](handl_plugin_codemod::EngineContextProvider)s
//!   — no concrete engine provider ships anywhere yet (a `typecheck`
//!   provider is design work: `hnl-tool-typecheck` pulls host deps the
//!   engine must not see, so it can only live plugin-side). Unregistered
//!   engines stay an honest `EngineUnavailable` diagnostic under the
//!   rule's `on_engine_error`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use handl_language::Language;
use handl_plugin_codemod::{Codemod, CodemodRewrite, CodemodSpec, resolve_targets};
use hnl_tool_codegen::{
    ApplyOptions, ApplyReport, ArtifactStore, Engine, FsStore, Manifest, RevertReport, RunMeta,
    journal_ref,
};
use hnl_tool_transform::native::NativeRewrite;
use serde::Serialize;

/// The lifecycle verb `hnl codemod` dispatches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verb {
    /// Dry-run: edits + Change set, no mutation (`--check`/`--dry-run`).
    Plan,
    /// Reconcile the store to the codemod's desired state (default).
    Apply,
    /// Undo a journal run's before-images — the only undo path.
    Revert,
}

impl Verb {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "plan" => Some(Self::Plan),
            "apply" => Some(Self::Apply),
            "revert" => Some(Self::Revert),
            _ => None,
        }
    }
}

/// Run the `hnl codemod` subcommand and return the process exit code.
///
/// `lang` is the canonical kit language — the host injects it because
/// this crate must not link `handl-kit-prelude` (same seam as
/// `codegen_command`). Equivalent to [`codemod_command_with`] with no
/// extra rewrite layers.
pub fn codemod_command(lang: &Language, args: &[String]) -> i32 {
    codemod_command_with(lang, &[], args)
}

/// [`codemod_command`] plus caller-supplied extra [`NativeRewrite`]
/// layers, forwarded to [`Codemod::with_rewrite_layer`] in order — the
/// seam hosts use to reach `handl:<mod>.<fn>` `.hnl` rewrite modules
/// (e.g. `Kit::handl_rewrite_adapter`) and `native:` functions the CLI
/// does not ship itself. The engine's internal `codemod.delete` layer
/// always stays ahead of these.
pub fn codemod_command_with(
    lang: &Language,
    rewrite_layers: &[Arc<dyn NativeRewrite>],
    args: &[String],
) -> i32 {
    let usage = "usage: hnl codemod [plan|apply|revert] <spec.yaml> [--root <dir>] [--target <glob>] [--package <dir>] [--json] [--check|--dry-run] [--run <id>] [--keep-going]";
    if args.is_empty() {
        eprintln!("{usage}");
        return 2;
    }

    // Leading verb, if present; otherwise `apply` (the `codegen`
    // convention) unless a plan flag appears below.
    let (mut verb, rest) = match Verb::parse(&args[0]) {
        Some(v) => (v, &args[1..]),
        None => (Verb::Apply, args),
    };
    if rest.is_empty() && verb != Verb::Revert {
        eprintln!("{usage}");
        return 2;
    }

    let mut positionals: Vec<String> = Vec::new();
    let mut root: Option<String> = None;
    let mut target_glob: Option<String> = None;
    let mut package: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut json = false;
    let mut check = false;
    let mut keep_going = false;

    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--root" => {
                i += 1;
                root = rest.get(i).map(String::to_string);
            }
            "--target" => {
                i += 1;
                target_glob = rest.get(i).map(String::to_string);
            }
            "--package" => {
                i += 1;
                package = rest.get(i).map(String::to_string);
            }
            "--run" => {
                i += 1;
                run_id = rest.get(i).map(String::to_string);
            }
            "--json" => json = true,
            "--check" | "--dry-run" | "--plan" => {
                if rest[i] == "--check" {
                    check = true;
                }
                if verb == Verb::Apply {
                    verb = Verb::Plan;
                } else if verb != Verb::Plan {
                    eprintln!("codemod: --check/--dry-run/--plan are only valid for apply/plan");
                    return 2;
                }
            }
            "--keep-going" => keep_going = true,
            s if s.starts_with("--") => {
                eprintln!("codemod: unknown flag `{s}`");
                return 2;
            }
            s => positionals.push(s.to_string()),
        }
        i += 1;
    }

    // Positionals: plan/apply take exactly `<spec>`; revert takes an
    // optional `<spec>` (its `name` locates the manifest's run id).
    let arity_ok = match verb {
        Verb::Revert => positionals.len() <= 1,
        _ => positionals.len() == 1,
    };
    if !arity_ok {
        eprintln!("{usage}");
        return 2;
    }
    let spec_arg = positionals.first().cloned();

    // Store root: `--root` wins; otherwise the spec's directory
    // (package-relative globs); revert without a spec falls back to cwd.
    let root_path: PathBuf = match (&root, &spec_arg) {
        (Some(r), _) => PathBuf::from(r),
        (None, Some(spec)) => Path::new(spec)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        (None, None) => PathBuf::from("."),
    };
    let root_path = root_path.canonicalize().unwrap_or(root_path);

    match verb {
        Verb::Revert => revert_command(
            &root_path,
            spec_arg.as_deref(),
            run_id.as_deref(),
            keep_going,
        ),
        Verb::Plan | Verb::Apply => run_spec_command(
            lang,
            rewrite_layers,
            verb,
            &root_path,
            spec_arg.as_deref().expect("arity"),
            SpecFlags {
                target_glob: target_glob.as_deref(),
                package: package.as_deref(),
                json,
                check,
                keep_going,
            },
        ),
    }
}

/// The non-positional flags `run_spec_command` consumes.
struct SpecFlags<'a> {
    /// `--target <glob>` artifact narrowing.
    target_glob: Option<&'a str>,
    /// `--package <dir>` — the package whose `.hnl` modules resolve
    /// `handl:<mod>.<fn>` rewrites; default is `pkg.hnl` discovery
    /// walking up from the spec path.
    package: Option<&'a str>,
    /// `--json` machine-readable plan output.
    json: bool,
    /// `--check`/`--dry-run` CI gate (plan + exit 1 on pending changes).
    check: bool,
    /// `--keep-going` per-artifact error policy.
    keep_going: bool,
}

/// plan/apply: load spec → merge provider rules → resolve globs →
/// `CodemodEngine::plan` → (apply) `Engine::apply`.
fn run_spec_command(
    lang: &Language,
    rewrite_layers: &[Arc<dyn NativeRewrite>],
    verb: Verb,
    root: &Path,
    spec_path: &str,
    flags: SpecFlags<'_>,
) -> i32 {
    let spec = match CodemodSpec::load(spec_path) {
        Ok(s) => s,
        // A missing spec file is a usage error (exit 2); a spec that
        // exists but does not parse/validate is a failure (exit 1).
        Err(handl_plugin_codemod::SpecError::Io { .. }) => {
            eprintln!("codemod: cannot read spec {spec_path}");
            return 2;
        }
        Err(e) => {
            eprintln!("codemod: {e}");
            return 1;
        }
    };

    let mut codemod = Codemod::new().with_kit_language(lang);
    for layer in rewrite_layers {
        codemod = codemod.with_rewrite_layer(Arc::clone(layer));
    }
    let spec = match codemod.merge_spec(&spec) {
        Ok(s) => s,
        Err(issues) => {
            for issue in &issues {
                eprintln!("codemod: spec issue: {issue}");
            }
            return 1;
        }
    };

    // Package modules (P-002/T-108): `handl:<mod>.<fn>` names resolve
    // against the `.hnl` modules of the package that owns the spec —
    // `--package <dir>` pins it, otherwise the nearest `pkg.hnl`
    // walking up from the spec path. Only attempted when the spec
    // names a `handl:` rewrite (or `--package` was given explicitly);
    // no package found leaves the name to the engine's honest
    // `RewriteError` diagnostic on the first match.
    let pkg_dir = match flags.package {
        Some(dir) => Some(PathBuf::from(dir)),
        None if spec_uses_handl_rewrites(&spec) => {
            handl_platform::discover_pkg_dir(Path::new(spec_path))
        }
        None => None,
    };
    if let Some(dir) = pkg_dir {
        let dir = dir.canonicalize().unwrap_or(dir);
        codemod = match codemod.with_package_modules(&dir) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "codemod: cannot load rewrite modules from package {}: {e}",
                    dir.display()
                );
                return 1;
            }
        };
    }

    let targets = match resolve_targets(root, &spec, flags.target_glob) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("codemod: {e}");
            return 1;
        }
    };

    let store = FsStore::new(root);
    let plan = match codemod.engine(&store).plan(&spec, &targets) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("codemod: {e}");
            return 1;
        }
    };

    if flags.json {
        println!("{}", plan_json(&plan));
    } else {
        print_plan(&plan);
    }

    match verb {
        Verb::Plan => {
            // `--check` is the CI gate: pending changes are a failure.
            if flags.check && !plan.changes.is_empty() {
                1
            } else {
                0
            }
        }
        Verb::Apply => {
            if plan.changes.is_empty() {
                println!("codemod apply {}: no changes", spec.name);
                return 0;
            }
            // prior = None deliberately: codemod never deletes via
            // manifest ownership (it does not own artifacts — a file
            // that stops matching keeps its content; `revert` is the
            // only undo).
            let mut meta = RunMeta::new(spec.name.clone(), spec.version.to_string());
            meta.new_run_id();
            let opts = if flags.keep_going {
                ApplyOptions::keep_going()
            } else {
                ApplyOptions::default()
            };
            let engine = Engine::new(&store);
            match engine.apply(&plan.changes, None, &meta, &opts) {
                Ok(report) => {
                    print_apply_report(&spec.name, &report);
                    if report.failures.is_empty() && report.fatal.is_none() && !report.aborted {
                        0
                    } else {
                        1
                    }
                }
                Err(e) => {
                    eprintln!("codemod: apply failed: {e}");
                    1
                }
            }
        }
        Verb::Revert => unreachable!(),
    }
}

/// Whether any rule names a `handl:<mod>.<fn>` rewrite — the trigger
/// for package-module discovery. `"handl:"` is the
/// `HandlNativeRewrite` prefix (`HANDL_PREFIX` in
/// `handl-plugin-lib-handl`, which this crate does not link).
fn spec_uses_handl_rewrites(spec: &CodemodSpec) -> bool {
    spec.rules.iter().any(|rule| {
        matches!(
            &rule.rewrite,
            Some(CodemodRewrite::Named(name)) if name.starts_with("handl:")
        )
    })
}

/// revert: `Engine::revert` on a journal run — `--run <id>` wins;
/// otherwise the spec's manifest supplies the run that produced the
/// current state. No spec + no `--run` is a usage error.
fn revert_command(root: &Path, spec_arg: Option<&str>, run: Option<&str>, keep_going: bool) -> i32 {
    let store = FsStore::new(root);
    let engine = Engine::new(&store);

    let run_id = match (run, spec_arg) {
        (Some(id), _) => id.to_string(),
        (None, Some(spec_path)) => {
            let spec = match CodemodSpec::load(spec_path) {
                Ok(s) => s,
                Err(handl_plugin_codemod::SpecError::Io { .. }) => {
                    eprintln!("codemod: cannot read spec {spec_path}");
                    return 2;
                }
                Err(e) => {
                    eprintln!("codemod: {e}");
                    return 1;
                }
            };
            match read_manifest_run_id(&store, &spec.name) {
                Ok(Some(id)) => id,
                Ok(None) => {
                    eprintln!(
                        "codemod: no manifest for '{}' under {} — pass --run <id>",
                        spec.name,
                        root.display()
                    );
                    return 1;
                }
                Err(e) => {
                    eprintln!("codemod: {e}");
                    return 1;
                }
            }
        }
        (None, None) => {
            eprintln!("codemod: revert needs <spec.yaml> or --run <id>");
            return 2;
        }
    };

    // A missing journal is an error, not a silent no-op: `Engine::revert`
    // on absent records succeeds trivially, which would lie about having
    // undone a run that never existed.
    let jref = journal_ref(&run_id);
    match store.read(&jref) {
        Ok(Some(_)) => {}
        Ok(None) => {
            eprintln!(
                "codemod: no journal for run '{run_id}' under {}",
                root.display()
            );
            return 1;
        }
        Err(e) => {
            eprintln!("codemod: cannot read journal {jref}: {e}");
            return 1;
        }
    }

    let opts = if keep_going {
        ApplyOptions::keep_going()
    } else {
        ApplyOptions::default()
    };
    match engine.revert(&run_id, &opts) {
        Ok(report) => {
            print_revert_report(&report);
            if report.failures.is_empty() && report.fatal.is_none() && !report.aborted {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("codemod: revert failed: {e}");
            1
        }
    }
}

/// The `run_id` recorded in `<name>`'s manifest, if a manifest exists
/// and decodes. A corrupt manifest is treated as absent.
fn read_manifest_run_id(store: &FsStore, name: &str) -> Result<Option<String>, String> {
    let manifest_ref = Manifest::artifact_ref(name);
    let bytes = store
        .read(&manifest_ref)
        .map_err(|e| format!("cannot read manifest {}: {e}", manifest_ref))?;
    match bytes {
        None => Ok(None),
        Some(bytes) => match serde_json::from_slice::<Manifest>(&bytes) {
            Ok(m) if !m.run_id.is_empty() => Ok(Some(m.run_id)),
            _ => Ok(None),
        },
    }
}

/// The JSON shape `--json` emits for `plan` (and the pre-apply half of
/// `apply`).
#[derive(Serialize)]
struct PlanJson<'a> {
    name: &'a str,
    version: u32,
    stats: &'a handl_plugin_codemod::CodemodStats,
    diagnostics: &'a [handl_plugin_codemod::CodemodDiagnostic],
    edits: &'a [handl_plugin_codemod::CodemodEdit],
    changes: Vec<ChangeJson>,
}

#[derive(Serialize)]
struct ChangeJson {
    artifact: String,
    op: &'static str,
    before_hash: Option<String>,
    after_hash: Option<String>,
}

fn plan_json(plan: &handl_plugin_codemod::CodemodPlan) -> String {
    let changes = plan
        .changes
        .iter()
        .map(|c| ChangeJson {
            artifact: c.artifact().to_string(),
            op: match c {
                hnl_tool_codegen::Change::Put { .. } => "put",
                hnl_tool_codegen::Change::Delete { .. } => "delete",
            },
            before_hash: c.before_hash().map(|h| h.to_hex()),
            after_hash: c.after_hash().map(|h| h.to_hex()),
        })
        .collect();
    let value = PlanJson {
        name: &plan.name,
        version: plan.version,
        stats: &plan.stats,
        diagnostics: &plan.diagnostics,
        edits: &plan.edits,
        changes,
    };
    serde_json::to_string_pretty(&value).expect("plan JSON is serializable")
}

/// Print a [`CodemodPlan`]: counts first, then one line per edit and
/// one per materialized change.
fn print_plan(plan: &handl_plugin_codemod::CodemodPlan) {
    println!(
        "codemod plan {}@{}: {} edit(s), {} change(s), {} diagnostic(s)",
        plan.name,
        plan.version,
        plan.edits.len(),
        plan.changes.len(),
        plan.diagnostics.len(),
    );
    for edit in &plan.edits {
        let span = edit
            .span
            .map(|s| format!("@{}..{}", s.start, s.end))
            .unwrap_or_default();
        let rewrite = edit
            .rewrite
            .as_deref()
            .map(|r| format!(" -> {r}"))
            .unwrap_or_default();
        println!(
            "  {} {} {}{}{}",
            edit.artifact,
            span,
            edit.rule_id,
            rewrite,
            if edit.deleted { " [delete]" } else { "" },
        );
    }
    for change in &plan.changes {
        println!("  ~ {}", change.artifact());
    }
    for diag in &plan.diagnostics {
        let at = diag
            .artifact
            .as_ref()
            .map(|a| format!(" {a}"))
            .unwrap_or_default();
        eprintln!("codemod: diagnostic{at}: {:?}: {}", diag.kind, diag.message);
    }
}

fn print_apply_report(name: &str, report: &ApplyReport) {
    println!(
        "codemod apply {name}: run {} — {} created, {} updated, {} deleted, {} unchanged, {} skipped, {} failed",
        report.run_id,
        report.created,
        report.updated,
        report.deleted,
        report.unchanged,
        report.skipped,
        report.failures.len(),
    );
    for (artifact, err) in &report.failures {
        eprintln!("codemod: {artifact}: {err}");
    }
    if let Some(fatal) = &report.fatal {
        eprintln!("codemod: {fatal}");
    }
}

fn print_revert_report(report: &RevertReport) {
    println!(
        "codemod revert: run {} — {} restored, {} removed, {} skipped, {} failed{}",
        report.run_id,
        report.restored,
        report.removed,
        report.skipped,
        report.failures.len(),
        if report.aborted { " (aborted)" } else { "" },
    );
    for (artifact, err) in &report.failures {
        eprintln!("codemod: {artifact}: {err}");
    }
    if let Some(fatal) = &report.fatal {
        eprintln!("codemod: {fatal}");
    }
}
