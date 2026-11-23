//! # handl-kit-prelude-lang
//!
//! The **canonical Language as data** — the eval-path fragment set of the
//! ADR-0044 §6 end state, split out of `handl-kit-data-prelude` so consumers
//! that only need to lex / parse / desugar / eval `.hnl` do not carry the
//! tooling fragments (`lints.yaml`, `lsp.yaml`) or the kit-composition
//! manifests (`data/kits/*`).
//!
//! Assembles a [`Language`] from **authored YAML data files** (one per
//! fragment type: `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `types.yaml`,
//! `ast.yaml`, `caps.yaml`, `options.yaml`, `lexer_rules.yaml`,
//! `transforms.yaml`, `globals.yaml`), embedded at compile time via
//! `include_str!`. No Rust emitter crates are needed — the language
//! definition is pure data.
//!
//! Tooling fragments merge on top: `handl-kit-data-prelude` composes this
//! crate's [`language_from_yaml`] and then merges `lints.yaml` / `lsp.yaml`
//! (plus the kit manifests) to produce the full canonical `Language` —
//! lints and lsp declarations occupy disjoint `lang.lints` / `lang.lsp`
//! sections that eval never consults.
//!
//! ## Usage
//!
//! ```no_run
//! let lang = handl_kit_prelude_lang::language_from_yaml().unwrap();
//! ```

#![warn(missing_docs)]

use std::path::Path;

use handl_language::{
    AstShapeDef, CapabilityDecl, EvalSchemeDef, GrammarRow, LangError, Language, LexerRuleDef,
    OptionSchema, TokenKindDef, TransformDef, TypeRuleDef,
};

/// Fragment ids that were excluded when generating the YAML files.
///
/// These are non-canonical duplicate fragments (behavioral, non-union types)
/// that share an identity key with a canonical fragment:
///
/// - `eval.call.postfix` — `call`'s `eval.call` is the canonical eval scheme.
/// - `grammar.call.postfix` — `call`'s `grammar.call.infix` is the canonical
///   infix rule for `(`.
/// - `grammar.apply.prefix` — `paren`'s `grammar.paren.prefix` is the
///   canonical prefix rule for `(`.
pub const EXCLUDED_FRAGMENT_IDS: &[&str] = &[
    "eval.call.postfix",
    "grammar.call.postfix",
    "grammar.apply.prefix",
];

// ---------------------------------------------------------------------------
// YAML data file loading
// ---------------------------------------------------------------------------

/// Embed a YAML data file at compile time.
const TOKENS_YAML: &str = include_str!("../data/tokens.yaml");
const GRAMMAR_YAML: &str = include_str!("../data/grammar.yaml");
const EVAL_YAML: &str = include_str!("../data/eval.yaml");
const TYPES_YAML: &str = include_str!("../data/types.yaml");
const AST_YAML: &str = include_str!("../data/ast.yaml");
const CAPS_YAML: &str = include_str!("../data/caps.yaml");
const OPTIONS_YAML: &str = include_str!("../data/options.yaml");
const LEXER_RULES_YAML: &str = include_str!("../data/lexer_rules.yaml");
const TRANSFORMS_YAML: &str = include_str!("../data/transforms.yaml");
const GLOBALS_YAML: &str = include_str!("../data/globals.yaml");

/// Merge ambient global value bindings (`globals.yaml`) into `lang`.
///
/// `globals.yaml` is a YAML mapping of `name: <literal value>` — the values
/// are data (numbers, strings, booleans, records), not evaluated source, so
/// no eval engine is needed at merge time. `Runtime` seeds them into the
/// engines' global scope at build time (`Language::globals`).
fn merge_globals(lang: &mut Language, globals_yaml: &str) -> Result<(), LangError> {
    let globals: std::collections::BTreeMap<String, handl_language::Value> =
        serde_yaml::from_str(globals_yaml)
            .map_err(|e| LangError::engine(format!("globals.yaml: {}", e)))?;
    lang.globals.extend(globals);
    Ok(())
}

/// Build the canonical eval-path [`Language`] from the embedded YAML files.
///
/// Each file contains a YAML list of fragments of one type. The fragments are
/// deserialized and merged into a single [`Language`]. `eval.yaml` holds the
/// arrow-calculus `Term` definitions (ADR-0052 kernel).
///
/// The full parse is cached on disk: the merged `Language` is serialized to
/// `~/.cache/handl-lang/language-<key>.bin`, keyed by a content hash of every
/// embedded fragment plus the package version. Reading the snapshot is ~100×
/// faster than re-parsing ~10k lines of YAML, which matters under runners
/// that spawn a process per test (nextest) — every test paid the parse.
/// A corrupt or stale snapshot simply falls back to parsing.
///
/// # Errors
///
/// Returns [`LangError`] if any YAML file fails to deserialize or a merge
/// fails (should not happen — the files are pre-resolved).
pub fn language_from_yaml() -> Result<Language, LangError> {
    let key = snapshot_key();
    if let Some(lang) = load_snapshot(key) {
        return Ok(lang);
    }
    let mut lang = merge_lang_fragments(
        TOKENS_YAML,
        GRAMMAR_YAML,
        EVAL_YAML,
        TYPES_YAML,
        AST_YAML,
        CAPS_YAML,
        OPTIONS_YAML,
        LEXER_RULES_YAML,
        TRANSFORMS_YAML,
    )?;
    merge_globals(&mut lang, GLOBALS_YAML)?;
    store_snapshot(key, &lang);
    Ok(lang)
}

/// Content hash of every embedded fragment plus the package version — any
/// data or layout change produces a new snapshot file instead of a stale hit.
fn snapshot_key() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut h);
    for frag in [
        TOKENS_YAML,
        GRAMMAR_YAML,
        EVAL_YAML,
        TYPES_YAML,
        AST_YAML,
        CAPS_YAML,
        OPTIONS_YAML,
        LEXER_RULES_YAML,
        TRANSFORMS_YAML,
        GLOBALS_YAML,
    ] {
        frag.hash(&mut h);
    }
    h.finish()
}

fn snapshot_path(key: u64) -> Option<std::path::PathBuf> {
    let base = std::env::var_os("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".cache"))
        .unwrap_or_else(std::env::temp_dir);
    Some(base.join("handl-lang").join(format!("language-{key:016x}.bin")))
}

fn load_snapshot(key: u64) -> Option<Language> {
    let path = snapshot_path(key)?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Write via tmp-file + rename so a concurrent reader never sees a torn
/// snapshot (nextest spawns test processes in parallel). Best-effort: a
/// failed store just means the next caller parses YAML again.
fn store_snapshot(key: u64, lang: &Language) {
    let Some(path) = snapshot_path(key) else { return };
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let Ok(bytes) = serde_json::to_vec(lang) else {
        return;
    };
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    if std::fs::write(&tmp, &bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Merge the eval-path YAML fragment strings into a single [`Language`].
///
/// Each argument is the raw YAML text for one fragment type — the eval-path
/// set only: tokens, grammar, eval, types, ast, caps, options, lexer_rules,
/// transforms. Tooling fragments (`lints`, `lsp`) are intentionally absent;
/// consumers that want them merge them on top (see `handl-kit-data-prelude`).
///
/// # Errors
///
/// Returns [`LangError`] if any YAML fails to deserialize or a merge fails.
pub fn merge_lang_fragments(
    tokens_yaml: &str,
    grammar_yaml: &str,
    eval_yaml: &str,
    types_yaml: &str,
    ast_yaml: &str,
    caps_yaml: &str,
    options_yaml: &str,
    lexer_rules_yaml: &str,
    transforms_yaml: &str,
) -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kinds (union — idempotent merge).
    let tokens: Vec<TokenKindDef> = serde_yaml::from_str(tokens_yaml)
        .map_err(|e| LangError::engine(format!("tokens.yaml: {}", e)))?;
    for frag in tokens {
        lang.tokens.merge(frag)?;
    }

    // Grammar rows (non-union — pre-resolved, no collisions).
    let grammar: Vec<GrammarRow> = serde_yaml::from_str(grammar_yaml)
        .map_err(|e| LangError::engine(format!("grammar.yaml: {}", e)))?;
    for frag in grammar {
        lang.grammar.merge(frag)?;
    }

    // Eval schemes (arrow-calculus kernel, ADR-0052; non-union — pre-resolved,
    // no collisions).
    let eval: Vec<EvalSchemeDef> = serde_yaml::from_str(eval_yaml)
        .map_err(|e| LangError::engine(format!("eval.yaml: {}", e)))?;
    for frag in eval {
        lang.eval.merge(frag)?;
    }

    // Type rules (ADR-0054 — non-union, pre-resolved).
    let types: Vec<TypeRuleDef> = serde_yaml::from_str(types_yaml)
        .map_err(|e| LangError::engine(format!("types.yaml: {}", e)))?;
    for frag in types {
        lang.types.merge(frag)?;
    }

    // AST shapes (union — idempotent merge). `merge_ast_fragment` also
    // lowers any `desugar:` rule each shape carries into a `TransformDef` in
    // `lang.transforms` (pass "desugar", ADR-0094 §6).
    let ast: Vec<AstShapeDef> = serde_yaml::from_str(ast_yaml)
        .map_err(|e| LangError::engine(format!("ast.yaml: {}", e)))?;
    for frag in ast {
        lang.merge_ast_fragment(frag)?;
    }

    // Capabilities (union — idempotent merge).
    let caps: Vec<CapabilityDecl> = serde_yaml::from_str(caps_yaml)
        .map_err(|e| LangError::engine(format!("caps.yaml: {}", e)))?;
    for frag in caps {
        lang.caps.merge(frag)?;
    }

    // Options (non-union).
    let options: Vec<OptionSchema> = serde_yaml::from_str(options_yaml)
        .map_err(|e| LangError::engine(format!("options.yaml: {}", e)))?;
    for frag in options {
        lang.options.merge(frag)?;
    }

    // Lexer rules (non-union).
    let lexer_rules: Vec<LexerRuleDef> = serde_yaml::from_str(lexer_rules_yaml)
        .map_err(|e| LangError::engine(format!("lexer_rules.yaml: {}", e)))?;
    for frag in lexer_rules {
        lang.lexer_rules.merge(frag)?;
    }

    // Transform rules (ADR-0094 — non-union, pre-resolved). Merged *after*
    // the ast pass so a first-class rule may `replaces:` a `desugar:`-lowered
    // `transform.desugar.*` rule synthesized above.
    let transforms: Vec<TransformDef> = serde_yaml::from_str(transforms_yaml)
        .map_err(|e| LangError::engine(format!("transforms.yaml: {}", e)))?;
    for frag in transforms {
        lang.transforms.merge(frag)?;
    }

    // ADR-0094 "Production readiness": transform-rule validation runs at
    // `Language` build. Error-severity diagnostics (duplicate identity keys,
    // out-of-bounds `Keep`/`KeepRole`/`KeepPath`/`KeepRest`/`Apply` steps,
    // dangling `Apply` rule ids, contradictory `KindAt` role/index pairs)
    // fail the merge; warnings (unknown target kinds, potential `Fixpoint`
    // cycles, `Native` rules pending a wired registry) do not — shapes and
    // registries may legitimately arrive from other fragments or engines.
    let diags = hnl_engine_transform::validate::validate_transforms(&lang);
    let errors: Vec<String> = diags
        .iter()
        .filter(|d| d.severity == handl_plugin_ext_diag::Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    if !errors.is_empty() {
        return Err(LangError::engine(format!(
            "transform rule validation failed: {}",
            errors.join("; ")
        )));
    }

    Ok(lang)
}

/// Merge lint-rule fragments into `lang` (ADR-0097 — non-union,
/// pre-resolved). Tooling-layer helper: `handl-kit-data-prelude` composes
/// this crate's language and then merges `lints.yaml` on top.
///
/// # Errors
///
/// Returns [`LangError`] if the YAML fails to deserialize or a merge fails.
pub fn merge_lints(lang: &mut Language, lints_yaml: &str) -> Result<(), LangError> {
    let lints: Vec<handl_language::LintRuleDef> = serde_yaml::from_str(lints_yaml)
        .map_err(|e| LangError::engine(format!("lints.yaml: {}", e)))?;
    for frag in lints {
        lang.lints.merge(frag)?;
    }
    Ok(())
}

/// Merge LSP-surface fragments into `lang` (ADR-0100 — non-union,
/// pre-resolved), then run the ADR-0100 LSP validation (error-severity
/// diagnostics fail the merge).
///
/// # Errors
///
/// Returns [`LangError`] if the YAML fails to deserialize, a merge fails, or
/// LSP validation reports errors.
pub fn merge_lsp(lang: &mut Language, lsp_yaml: &str) -> Result<(), LangError> {
    let lsp_frags: Vec<handl_language::LspDef> = serde_yaml::from_str(lsp_yaml)
        .map_err(|e| LangError::engine(format!("lsp.yaml: {}", e)))?;
    for frag in lsp_frags {
        lang.lsp.merge(frag)?;
    }

    // ADR-0100: LSP surface validation runs at Language build.
    let lsp_diags = hnl_engine_transform::validate::validate_lsp(lang);
    let lsp_errors: Vec<String> = lsp_diags
        .iter()
        .filter(|d| d.severity == handl_plugin_ext_diag::Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    if !lsp_errors.is_empty() {
        return Err(LangError::engine(format!(
            "lsp rule validation failed: {}",
            lsp_errors.join("; ")
        )));
    }
    Ok(())
}

/// Build a [`Language`] from the eval-path YAML data files in a directory.
///
/// Reads `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `types.yaml`,
/// `ast.yaml`, `caps.yaml`, `options.yaml`, `lexer_rules.yaml` and
/// (optionally) `transforms.yaml` / `globals.yaml` from `dir` and merges
/// them the same way [`language_from_yaml`] merges the embedded files.
/// `lints.yaml` / `lsp.yaml` are tooling fragments — layer them via
/// [`merge_lints`] / [`merge_lsp`] when the consumer wants them.
///
/// # Errors
///
/// Returns [`LangError`] if `dir` is not a directory, any required YAML file
/// is missing or unreadable, or a fragment fails to deserialize/merge.
pub fn language_from_dir(dir: &Path) -> Result<Language, LangError> {
    if !dir.is_dir() {
        return Err(LangError::engine(format!(
            "language_from_dir: {:?} is not a directory",
            dir
        )));
    }

    let read = |name: &str| -> Result<String, LangError> {
        std::fs::read_to_string(dir.join(name))
            .map_err(|e| LangError::engine(format!("{}: {}", name, e)))
    };

    let tokens_yaml = read("tokens.yaml")?;
    let grammar_yaml = read("grammar.yaml")?;
    let eval_yaml = read("eval.yaml")?;
    let types_yaml = read("types.yaml")?;
    let ast_yaml = read("ast.yaml")?;
    let caps_yaml = read("caps.yaml")?;
    let options_yaml = read("options.yaml")?;
    let lexer_rules_yaml = read("lexer_rules.yaml")?;

    // `transforms.yaml` is optional: fragment directories written before
    // ADR-0094's first-class transform section do not carry one.
    let transforms_yaml = match std::fs::read_to_string(dir.join("transforms.yaml")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "[]".to_string(),
        Err(e) => return Err(LangError::engine(format!("transforms.yaml: {}", e))),
    };

    // `globals.yaml` is optional: fragment directories written before
    // ambient globals do not carry one.
    let globals_yaml = match std::fs::read_to_string(dir.join("globals.yaml")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
        Err(e) => return Err(LangError::engine(format!("globals.yaml: {}", e))),
    };

    let mut lang = merge_lang_fragments(
        &tokens_yaml,
        &grammar_yaml,
        &eval_yaml,
        &types_yaml,
        &ast_yaml,
        &caps_yaml,
        &options_yaml,
        &lexer_rules_yaml,
        &transforms_yaml,
    )?;
    merge_globals(&mut lang, &globals_yaml)?;
    Ok(lang)
}
