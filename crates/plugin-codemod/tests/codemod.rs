//! Integration tests for `handl-plugin-codemod` (PL-004 T-085):
//! glob resolution, spec/provider merging, and the wired
//! `CodemodEngine` over `MemoryStore`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use handl_kit_data_prelude::assemble;
use handl_language::{AstNode, AstRef, Language};
use handl_plugin_codemod::{
    Codemod, CodemodRule, CodemodRuleProvider, CodemodSpec, CodemodTarget, RuleScope,
    resolve_targets,
};
use hnl_engine_codegen::{ArtifactRef, ArtifactStore, MemoryStore};
use hnl_engine_codemod::{
    CodemodRewrite, EngineContextError, EngineContextProvider, EngineContextReport,
    EngineContextRequest, EngineErrorPolicy, TargetArtifacts, UnparseMode,
};
use hnl_engine_transform::native::RewriteCtx;
use serde_json::json;

fn lang() -> Language {
    assemble().unwrap().lang().clone()
}

fn spec_with(glob: &str, rules: Vec<CodemodRule>) -> CodemodSpec {
    CodemodSpec {
        name: "t".into(),
        version: 1,
        targets: vec![CodemodTarget {
            language: "handl".into(),
            glob: glob.into(),
            unparse: UnparseMode::Preserve,
        }],
        rules,
    }
}

fn delete_assign_rule(id: &str) -> CodemodRule {
    CodemodRule {
        id: id.into(),
        scope: RuleScope::File,
        requires: vec![],
        on_engine_error: EngineErrorPolicy::Skip,
        condition: Some(json!("assign")),
        rewrite: Some(CodemodRewrite::Delete { delete: true }),
    }
}

// --- resolve_targets ----------------------------------------------------

#[test]
fn resolve_targets_matches_glob_and_skips_kernel_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src/deep")).unwrap();
    std::fs::create_dir_all(root.join(".handl/codegen/journal")).unwrap();
    std::fs::create_dir_all(root.join(".handl/codegen/manifest")).unwrap();
    std::fs::create_dir_all(root.join("other")).unwrap();
    std::fs::write(root.join("src/a.hnl"), "A := 1\n").unwrap();
    std::fs::write(root.join("src/deep/b.hnl"), "B := 2\n").unwrap();
    std::fs::write(root.join("other/c.hnl"), "C := 3\n").unwrap();
    // Kernel bookkeeping must never become a codemod target even when
    // the glob would match it.
    std::fs::write(root.join(".handl/codegen/journal/run-1.hnl"), "x\n").unwrap();
    std::fs::write(root.join(".handl/codegen/manifest/t.hnl"), "x\n").unwrap();

    let spec = spec_with("src/**/*.hnl", vec![]);
    let targets = resolve_targets(root, &spec, None).unwrap();
    assert_eq!(targets.len(), 1);
    let refs: Vec<&str> = targets[0].artifacts.iter().map(|a| a.as_str()).collect();
    // `src/**/*.hnl` covers both depths (glob `**` spans zero+ dirs);
    // the `.handl/codegen/` files are excluded by the reserved prefix.
    assert_eq!(refs, ["src/a.hnl", "src/deep/b.hnl"]);
}

#[test]
fn resolve_targets_extra_glob_narrows_and_parent_escape_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.hnl"), "A := 1\n").unwrap();
    std::fs::write(root.join("src/b.hnl"), "B := 2\n").unwrap();
    // A file outside the root that a `..` glob could reach.
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("escape.hnl"), "E := 0\n").unwrap();

    let spec = spec_with("src/*.hnl", vec![]);
    let narrowed = resolve_targets(root, &spec, Some("src/b.hnl")).unwrap();
    let refs: Vec<&str> = narrowed[0].artifacts.iter().map(|a| a.as_str()).collect();
    assert_eq!(refs, ["src/b.hnl"]);

    // A `..` glob must never yield artifacts outside the root — the
    // lexical `strip_prefix` leaves `..` components that are rejected.
    let escape_glob = format!(
        "../{}/*.hnl",
        outside.path().file_name().unwrap().to_string_lossy()
    );
    let escaped = resolve_targets(root, &spec_with(&escape_glob, vec![]), None).unwrap();
    assert!(
        escaped[0].artifacts.is_empty(),
        "got {:?}",
        escaped[0].artifacts
    );
}

#[test]
fn resolve_targets_reports_invalid_glob() {
    let dir = tempfile::tempdir().unwrap();
    let spec = spec_with("src/**/[bad", vec![]);
    let err = resolve_targets(dir.path(), &spec, None).unwrap_err();
    assert!(err.contains("targets[0].glob"), "got {err}");
}

// --- merge_spec / providers --------------------------------------------

struct ExtraRuleProvider {
    id: &'static str,
    rule_id: &'static str,
}

impl CodemodRuleProvider for ExtraRuleProvider {
    fn id(&self) -> &str {
        self.id
    }
    fn rules(&self) -> Vec<CodemodRule> {
        vec![delete_assign_rule(self.rule_id)]
    }
}

#[test]
fn merge_spec_appends_provider_rules() {
    let codemod = Codemod::new().with_rule_provider(Arc::new(ExtraRuleProvider {
        id: "pack-a",
        rule_id: "del-b",
    }));
    let spec = spec_with("**/*.hnl", vec![delete_assign_rule("del-a")]);
    let merged = codemod.merge_spec(&spec).unwrap();
    let ids: Vec<&str> = merged.rules.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["del-a", "del-b"]);
}

#[test]
fn merge_spec_rejects_duplicate_rule_ids() {
    let codemod = Codemod::new().with_rule_provider(Arc::new(ExtraRuleProvider {
        id: "pack-a",
        rule_id: "del-a", // collides with the spec's own rule
    }));
    let spec = spec_with("**/*.hnl", vec![delete_assign_rule("del-a")]);
    let issues = codemod.merge_spec(&spec).unwrap_err();
    assert!(
        issues
            .iter()
            .any(|i| i.message.contains("duplicate rule id"))
    );
}

// --- engine wiring ------------------------------------------------------

/// A `requires:` provider that counts invocations — proves contexts
/// are wired through `Codemod::with_engine_context`.
struct CountingContext {
    calls: Arc<AtomicUsize>,
}

impl EngineContextProvider for CountingContext {
    fn engine_name(&self) -> &str {
        "probe"
    }
    fn engine_version(&self) -> &str {
        "1"
    }
    fn provide(
        &self,
        _req: &EngineContextRequest<'_>,
    ) -> Result<EngineContextReport, EngineContextError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(EngineContextReport {
            engine: "probe".into(),
            version: "1".into(),
            report: json!({"ok": true}),
        })
    }
}

#[test]
fn engine_plan_runs_delete_rewrite_end_to_end() {
    let store = MemoryStore::new();
    store
        .write(&ArtifactRef::new("a.hnl"), b"A := 1\nB := 2\n")
        .unwrap();
    let spec = spec_with("**/*.hnl", vec![delete_assign_rule("del")]);
    let targets = vec![TargetArtifacts {
        target: 0,
        artifacts: vec![ArtifactRef::new("a.hnl")],
    }];

    let codemod = Codemod::new().with_kit_language(&lang());
    let plan = codemod.engine(&store).plan(&spec, &targets).expect("plan");
    assert_eq!(plan.edits.len(), 2, "both assigns matched");
    assert!(plan.edits.iter().all(|e| e.deleted));
    assert_eq!(plan.changes.len(), 1, "one rewritten artifact");
    let text = plan
        .changes
        .iter()
        .find_map(|c| c.content().map(|b| String::from_utf8_lossy(b).into_owned()))
        .expect("put content");
    assert!(!text.contains(":="), "assigns deleted, got {text:?}");
}

#[test]
fn engine_plan_with_native_rewrite_and_context() {
    let store = MemoryStore::new();
    store
        .write(&ArtifactRef::new("a.hnl"), b"A := 1\n")
        .unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    let rule = CodemodRule {
        id: "v99".into(),
        scope: RuleScope::File,
        requires: vec!["probe".into()],
        on_engine_error: EngineErrorPolicy::Fail,
        condition: Some(json!("value")),
        rewrite: Some(CodemodRewrite::Named("native:v99".into())),
    };

    let spec = CodemodSpec {
        name: "t".into(),
        version: 1,
        targets: vec![CodemodTarget {
            language: "handl".into(),
            glob: "**/*.hnl".into(),
            unparse: UnparseMode::Preserve,
        }],
        rules: vec![rule],
    };
    let targets = vec![TargetArtifacts {
        target: 0,
        artifacts: vec![ArtifactRef::new("a.hnl")],
    }];

    let codemod = Codemod::new()
        .with_kit_language(&lang())
        .with_native("v99", |ctx: &mut RewriteCtx<'_>| {
            let idx = ctx
                .out()
                .push(AstNode::new("value").with_payload(json!(99)))
                .index();
            Ok(AstRef::new(idx))
        })
        .with_engine_context(Arc::new(CountingContext {
            calls: Arc::clone(&calls),
        }));
    let plan = codemod.engine(&store).plan(&spec, &targets).expect("plan");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "context provided once");
    assert!(plan.edits.iter().any(|e| e.rule_id == "v99"));
}

#[test]
fn unresolved_language_is_an_honest_diagnostic() {
    let store = MemoryStore::new();
    store
        .write(&ArtifactRef::new("a.hnl"), b"A := 1\n")
        .unwrap();
    let mut spec = spec_with("**/*.hnl", vec![]);
    spec.targets[0].language = "nonexistent".into();
    let targets = vec![TargetArtifacts {
        target: 0,
        artifacts: vec![ArtifactRef::new("a.hnl")],
    }];
    let codemod = Codemod::new().with_kit_language(&lang());
    let plan = codemod.engine(&store).plan(&spec, &targets).expect("plan");
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.kind == handl_plugin_codemod::DiagnosticKind::LanguageUnresolved),
        "diagnostics: {:?}",
        plan.diagnostics
    );
    assert!(plan.changes.is_empty());
}

#[test]
fn spec_load_roundtrip_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("codemod.yaml");
    std::fs::write(
        &path,
        "name: rt\nversion: 2\ntargets:\n  - language: handl\n    glob: \"src/**/*.hnl\"\n    unparse: preserve\nrules:\n  - id: r\n    match: \"assign\"\n    rewrite: { delete: true }\n",
    )
    .unwrap();
    let spec = CodemodSpec::load(&path).expect("load");
    assert_eq!(spec.name, "rt");
    assert_eq!(spec.version, 2);
    assert_eq!(spec.rules.len(), 1);
}
