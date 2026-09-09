//! Bootstrap test runner (ADR-0089).
//!
//! Discovers, runs, and reports tests written in handl. The runner parses
//! modules with the canonical kit's Pratt parser, extracts test declarations
//! (bindings whose name starts with `test_` or test-language `test`/`property`
//! etc. forms), and evaluates each test body with the tree-walk eval engine.
//! Failures surface as `result.err(...)` values from the keyword `assert`
//! form; the runner catches those and records structured failure detail.
//!
//! # Test declaration convention
//!
//! The v0.1 grammar (ADR-0074) does not yet have a `test` keyword. The runner
//! uses a **naming convention**: any top-level binding whose name starts with
//! `test_` is treated as a test case. The test body is the binding's value
//! expression. For example:
//!
//! ```handl
//! test_addition = 1 + 1
//! test_failure = assert false
//! ```
//!
//! The first test passes (the expression evaluates without error); the second
//! fails (the assertion returns a `result.err` value). If the grammar later gains a `test`
//! keyword, the runner also checks for node kinds containing `"test"` (see
//! [`discover_tests_in_module`]).
//!
//! # Determinism
//!
//! Tests run in source order per module; modules are processed in lexicographic
//! order by path. No shared state between tests (handl has no mutation
//! primitive — this falls out of the language).
//!
//! # Engine genericity (ADR-0081)
//!
//! Child positions are resolved via [`Language::child_role_index`], never by
//! hardcoded `kind == "assign"` checks. The runner resolves the `target` and
//! `value` roles for each binding node; it falls back to positional children
//! (0 = target, 1 = value) only when no roles are declared.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use handl_language::{
    Ast, AstNode, AstRef, AstShapeDef, EvalSchemeDef, GrammarRow, Language, LexerRuleDef, Source,
    TokenKindDef, TransformDef, Value, as_tag, language_value, tag_value,
};

use handl_kit_canonical::handl_module::{HandlModule, ModuleResolver};
use handl_platform::{EvalContext, Function, Module, ModuleLookup};
use handl_plugin_engine_exec_tree_walk::{AssertFailure, AssertHandler, TreeWalkEngine};
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse::ParserEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_exec_cache::AstKey;
use handl_plugin_ext_diag::{Span, StatementChunk};
use handl_plugin_ext_refinement::{Dialect, detect_dialect};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_scheduler::{BoundedScheduler, JobError, SchedulePolicy};
use handl_runtime::Runtime;

use crate::events::TestEvent;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Handler that turns a failed `assert` keyword evaluation into a
/// `result.err(...)` value and a [`FailureDetail`] stored on the worker
/// thread.
#[derive(Debug, Clone, Copy)]
struct TestAssertHandler;

impl AssertHandler for TestAssertHandler {
    fn on_failure(&self, failure: AssertFailure) -> Value {
        crate::framework::set_last_assert_failure(FailureDetail {
            assertion: Some(failure.assertion.clone()),
            expected: failure.expected.clone(),
            actual: failure.actual.clone(),
            diff: failure.diff.clone(),
            cause: None,
        });
        let payload = serde_json::json!({
            "message": failure.message,
            "assertion": failure.assertion,
            "expected": failure.expected,
            "actual": failure.actual,
            "diff": failure.diff,
        });
        tag_value("err", &serde_json::json!({"value": payload}))
            .expect("err tag value must not contain __tag__")
    }
}

/// A discovered test case.
#[derive(Debug, Clone)]
pub struct TestCase {
    /// Module file path.
    pub module: PathBuf,
    /// Test name.
    pub name: String,
    /// Source text of the test body (the expression to evaluate).
    pub body_source: String,
    /// Source span of the test declaration.
    pub span: Span,
    /// Tags extracted from leading `-- tags: ...` doc-comment lines.
    pub tags: Vec<String>,
}

/// A test plan: all discovered tests, grouped by module.
#[derive(Debug, Clone, Default)]
pub struct TestPlan {
    /// All test cases, in source order per module, modules in lexicographic
    /// order.
    pub tests: Vec<TestCase>,
    /// Modules that failed to parse (discovery errors). Each entry is
    /// (path, error message).
    pub parse_errors: Vec<(PathBuf, String)>,
}

/// Error from test discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoverError {
    /// A file could not be read.
    Io(String),
    /// A path does not exist.
    NotFound(String),
}

impl std::fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoverError::Io(msg) => write!(f, "io error: {msg}"),
            DiscoverError::NotFound(msg) => write!(f, "path not found: {msg}"),
        }
    }
}

impl std::error::Error for DiscoverError {}

/// Result of running a single test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestResult {
    /// Test passed.
    Pass,
    /// Test failed with an error message.
    Fail {
        /// The error message from the raised error.
        message: String,
    },
    /// Test was skipped (filtered out).
    Skip,
}

/// Structured failure detail (ADR-0089 §"Structured failures").
///
/// Carries the ADR's failure fields beyond the error message: the failed
/// `assertion`, `expected`/`actual` values, a rendered `diff`, and an
/// underlying `cause`. The test's identity and source `location` live on
/// [`TestOutcome::case`] and the error message on
/// [`TestResult::Fail::message`]; `duration` lives on [`TestOutcome`].
///
/// The `assert` keyword handler stores these fields in a thread-local
/// side-channel when the assertion fails; `eval_test_body` consumes the
/// channel on failure and returns it alongside the result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureDetail {
    /// The assertion that failed (e.g. `assert`), when known.
    pub assertion: Option<String>,
    /// The expected value, rendered, when known.
    pub expected: Option<String>,
    /// The actual value, rendered, when known.
    pub actual: Option<String>,
    /// A rendered diff between expected and actual, when available.
    pub diff: Option<String>,
    /// An underlying cause, when the error carries one.
    pub cause: Option<String>,
}

/// A single test's outcome with metadata.
#[derive(Debug, Clone)]
pub struct TestOutcome {
    /// The test case.
    pub case: TestCase,
    /// The result.
    pub result: TestResult,
    /// Measured execution duration. Parallel-safe: taken inside each
    /// scheduler job around the body evaluation. Zero for skipped tests
    /// and cache replays.
    pub duration: Duration,
    /// Structured failure detail; empty unless the error carried the
    /// fields (see [`FailureDetail`]).
    pub failure: FailureDetail,
}

/// A run report: outcomes for all tests in a plan.
#[derive(Debug, Clone, Default)]
pub struct RunReport {
    /// Outcomes in execution order.
    pub outcomes: Vec<TestOutcome>,
}

impl RunReport {
    /// Number of passed tests.
    pub fn passed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.result == TestResult::Pass)
            .count()
    }

    /// Number of failed tests.
    pub fn failed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.result, TestResult::Fail { .. }))
            .count()
    }

    /// Number of skipped tests.
    pub fn skipped(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.result == TestResult::Skip)
            .count()
    }

    /// Total number of tests.
    pub fn total(&self) -> usize {
        self.outcomes.len()
    }

    /// True iff any test failed.
    pub fn has_failures(&self) -> bool {
        self.outcomes
            .iter()
            .any(|o| matches!(o.result, TestResult::Fail { .. }))
    }
}

/// Internal plan-state for a test during a run.
#[derive(Debug, Clone)]
enum TestState {
    /// The test was filtered out.
    Skip,
    /// The test was served from the result cache.
    Cached(TestResult, FailureDetail),
    /// The test will be executed by the scheduler.
    Run,
}

// ---------------------------------------------------------------------------
// Source-chunk splitting
// ---------------------------------------------------------------------------

/// A top-level statement chunk (alias for the shared
/// [`StatementChunk`](handl_plugin_ext_diag::StatementChunk)).
type Chunk = StatementChunk;

/// Split `src` into top-level statement chunks.
///
/// The v0.1 surface separates top-level declarations by newlines or
/// semicolons. The split itself is the shared `plugin-ext-diag`
/// implementation (AGENTS.md "Source splitting"); the runner additionally
/// drops import declarations (`from ... import ...` / `import ...`), which
/// are not valid bare expressions — the test runtime already registers
/// stdlib modules (str, fn, dedent, etc.) via the kit's module chain, so
/// test files don't need imports.
fn split_statements(src: &str) -> Vec<Chunk> {
    handl_plugin_ext_diag::split_statements(src)
        .into_iter()
        .filter(|chunk| !is_import_decl(&chunk.text))
        .collect()
}

/// Whether a chunk's text is an import declaration.
fn is_import_decl(trimmed: &str) -> bool {
    trimmed.starts_with("from ")
        || trimmed.starts_with("from\t")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("import\t")
        || trimmed.starts_with("import\"")
        || trimmed.starts_with("import'")
}

/// Extract tags declared in `-- tags: ...` doc-comment lines immediately
/// preceding a statement.
///
/// Tags are declared on comment lines above a declaration, e.g.
/// `-- tags: slow,net`. Multiple `-- tags:` lines accumulate top-down. A
/// blank line, non-comment line, or non-tag comment line stops the scan so
/// unrelated comments are not attached.
fn extract_doc_tags(src: &str, stmt_offset: usize) -> Vec<String> {
    let prefix = &src[..stmt_offset.min(src.len())];
    let lines: Vec<&str> = prefix.split('\n').collect();
    let mut tag_lines: Vec<Vec<String>> = Vec::new();
    // Scan bottom-up, skipping the partial line the statement starts on.
    for line in lines.into_iter().rev().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let Some(body) = trimmed.strip_prefix("--") else {
            break;
        };
        let body = body.trim_start();
        let Some(rest) = body
            .strip_prefix("tags:")
            .or_else(|| body.strip_prefix("@tags:"))
        else {
            break;
        };
        let mut tags = Vec::new();
        for raw in rest.split(|c: char| c == ',' || c.is_whitespace()) {
            let t = raw.trim();
            if !t.is_empty() {
                tags.push(t.to_string());
            }
        }
        tag_lines.push(tags);
    }
    // Return tags in source order (top-down), preserving within-line order.
    tag_lines.into_iter().rev().flatten().collect()
}

// ---------------------------------------------------------------------------
// AST helpers (data-driven child resolution, ADR-0081)
// ---------------------------------------------------------------------------

/// Extract the `children` index array from a node's payload.
fn children_of(node: &AstNode) -> Vec<usize> {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default()
}

/// The name carried by a parameter node: a bare `var` or a `type-annot`
/// wrapping one. Resolved through `Language::param_name` using the
/// `pattern_role: leaf` / `subject` role metadata (ADR-0081).
fn var_name(lang: &Language, ast: &Ast, node: &AstNode) -> Option<String> {
    let is_name = lang
        .ast
        .get_by_identity(&node.kind)
        .and_then(|s| s.pattern_role.as_deref())
        == Some("leaf")
        || lang
            .child_role_index(&node.kind, "subject")
            .ok()
            .flatten()
            .is_some();
    if !is_name {
        return None;
    }
    lang.param_name(ast, node).ok()
}

/// Resolve the binding's target (name) and value (expression) child indices
/// using named roles (ADR-0081), falling back to positional (0, 1) when no
/// roles are declared.
fn binding_roles(lang: &Language, kind: &str) -> (Option<usize>, Option<usize>) {
    let target_idx = lang.child_role_index(kind, "target").ok().flatten();
    let value_idx = lang.child_role_index(kind, "value").ok().flatten();
    if target_idx.is_some() && value_idx.is_some() {
        return (target_idx, value_idx);
    }
    // Fall back to positional: child 0 = target, child 1 = value.
    (Some(0), Some(1))
}

// ---------------------------------------------------------------------------
// Test-language fragment (ADR-0089 §"Test declaration syntax")
// ---------------------------------------------------------------------------

/// The test-language fragment YAML, embedded at compile time.
///
/// Declares `test`/`property`/`integration_test`/`bench` `AstShapeDef`s
/// (with `name`/`body` roles), `GrammarRow`s, `LexerRule`s, and
/// `EvalSchemeDef`s. Merged onto the canonical kit via [`Language::merge`]
/// (ADR-0044 language-as-data) — the ADR-0074 core grammar is not amended.
const TEST_LANGUAGE_YAML: &str = include_str!("../data/test-language.yaml");

/// Deserialized view of [`TEST_LANGUAGE_YAML`]'s top-level sections.
///
/// The fragment file declares each fragment type as a YAML list under a
/// named section; each list is merged into a fresh [`Language`] fragment.
#[derive(Debug, Deserialize)]
struct TestLanguageYaml {
    /// `tokens:` — keyword token kinds (`test`, `property`, ...).
    #[serde(default)]
    tokens: Vec<TokenKindDef>,
    /// `lexer_rules:` — literal patterns for the new keywords.
    #[serde(default)]
    lexer_rules: Vec<LexerRuleDef>,
    /// `grammar:` — prefix grammar rows for the declaration forms.
    #[serde(default)]
    grammar: Vec<GrammarRow>,
    /// `ast:` — `AstShapeDef`s declaring the `name`/`body` child roles.
    #[serde(default)]
    ast: Vec<AstShapeDef>,
    /// `eval:` — eval schemes that project the `body` child.
    #[serde(default)]
    eval: Vec<EvalSchemeDef>,
    /// `transforms:` — first-class `TransformDef` rules (ADR-0094).
    #[serde(default)]
    transforms: Vec<TransformDef>,
}

/// Build the test-language fragment as a standalone [`Language`].
///
/// # Errors
///
/// Returns an error string if the embedded YAML fails to deserialize or a
/// fragment merge fails (should not happen — the fragment is self-contained).
fn test_language_fragment() -> Result<Language, String> {
    let doc: TestLanguageYaml =
        serde_yaml::from_str(TEST_LANGUAGE_YAML).map_err(|e| format!("test-language.yaml: {e}"))?;
    let mut frag = Language::new();
    for f in doc.tokens {
        frag.tokens.merge(f).map_err(|e| e.to_string())?;
    }
    for f in doc.lexer_rules {
        frag.lexer_rules.merge(f).map_err(|e| e.to_string())?;
    }
    for f in doc.grammar {
        frag.grammar.merge(f).map_err(|e| e.to_string())?;
    }
    for f in doc.ast {
        frag.ast.merge(f).map_err(|e| e.to_string())?;
    }
    for f in doc.eval {
        frag.eval.merge(f).map_err(|e| e.to_string())?;
    }
    for f in doc.transforms {
        frag.transforms.merge(f).map_err(|e| e.to_string())?;
    }
    Ok(frag)
}

/// Build the **test language**: canonical kit ⊕ test-language fragment.
///
/// Both discovery and execution use this merged language so that
/// `test "name" = body`-style declarations parse and evaluate identically
/// in both stages (ADR-0089, ADR-0044).
///
/// # Errors
///
/// Returns an error string if the canonical kit or the merge fails.
fn test_language() -> Result<Language, String> {
    let kit = handl_kit_canonical::Kit::new()
        .map_err(|e| format!("failed to assemble canonical kit: {e}"))?;
    let mut lang = kit.runtime().lang().clone();
    lang.merge(test_language_fragment()?)
        .map_err(|e| format!("failed to merge test-language fragment: {e}"))?;
    Ok(lang)
}

/// Resolve the `name`/`body` child indices for a test-declaration node kind.
///
/// A node kind whose `AstShapeDef` declares **both** `name` and `body` roles
/// is a test declaration (the test-language fragment's `test`, `property`,
/// `integration_test`, `bench` shapes). This is resolved by role, never by
/// `kind ==` checks (ADR-0081): adding a new test-kind is a fragment edit,
/// not a runner edit.
fn test_decl_roles(lang: &Language, kind: &str) -> Option<(usize, usize)> {
    let name_idx = lang.child_role_index(kind, "name").ok().flatten()?;
    let body_idx = lang.child_role_index(kind, "body").ok().flatten()?;
    Some((name_idx, body_idx))
}

// ---------------------------------------------------------------------------
// Test discovery
// ---------------------------------------------------------------------------

/// Parse a single statement chunk into an [`Ast`].
fn parse_chunk(lang: &Language, text: &str) -> Result<Ast, String> {
    let tokens = handl_plugin_engine_lex_pattern::lex(lang, text).map_err(|e| e.to_string())?;
    if tokens.as_slice().is_empty() {
        return Ok(Ast::new());
    }
    let req = handl_plugin_engine_parse::ParserRequest::new(tokens);
    PrattEngine::new()
        .parse(lang, &req)
        .map_err(|e| e.to_string())
}

/// Discover tests in a single module's source text.
///
/// Walks top-level statements, extracting bindings whose name starts with
/// `test_`. Also checks for node kinds containing `"test"` (future grammar
/// support). Returns test cases in source order.
fn discover_tests_in_module(
    lang: &Language,
    src: &str,
    module: &Path,
) -> Result<Vec<TestCase>, String> {
    let chunks = split_statements(src);
    let mut tests = Vec::new();

    for chunk in &chunks {
        let ast = parse_chunk(lang, &chunk.text)?;
        if ast.is_empty() {
            continue;
        }
        let root_idx = ast.len() - 1;
        let root = match ast.at(AstRef::new(root_idx)) {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Test-language declaration: a node kind declaring both `name` and
        // `body` roles (`test`, `property`, `integration_test`, `bench` —
        // resolved via `child_role_index`, ADR-0081).
        if let Some((name_idx, _body_idx)) = test_decl_roles(lang, &root.kind) {
            let children = children_of(root);
            if let Some(&name_raw) = children.get(name_idx) {
                if let Ok(name_node) = ast.at(AstRef::new(name_raw)) {
                    if let Some(name) = var_name(lang, &ast, name_node) {
                        // The body expression is the text after the `=`
                        // separator in the declaration chunk.
                        let body_source = extract_body_source(&chunk.text);
                        let span = Span::from_byte_range(
                            chunk.offset,
                            chunk.offset + chunk.text.len(),
                            src,
                        );
                        let tags = extract_doc_tags(src, chunk.offset);
                        tests.push(TestCase {
                            module: module.to_path_buf(),
                            name,
                            body_source,
                            span,
                            tags,
                        });
                    }
                }
            }
            continue;
        }

        // Check for a `test` node kind (future grammar support).
        let is_test_kind = root.kind.contains("test");

        // Binding: a node with `target` + `value` roles (data-driven —
        // any shape declaring these roles is a binding, not just `assign`).
        let (target_idx, value_idx) = binding_roles(lang, &root.kind);
        if let (Some(ti), Some(vi)) = (target_idx, value_idx) {
            let children = children_of(root);
            if let (Some(&target_raw), Some(&value_raw)) = (children.get(ti), children.get(vi)) {
                if let (Ok(target_node), Ok(_value_node)) = (
                    ast.at(AstRef::new(target_raw)),
                    ast.at(AstRef::new(value_raw)),
                ) {
                    if let Some(name) = var_name(lang, &ast, target_node) {
                        // Test convention: name starts with `test_` or the
                        // node kind contains "test".
                        if name.starts_with("test_") || is_test_kind {
                            // Extract the body source: the text after `=`
                            // in the chunk, or the whole chunk if no `=`.
                            let body_source = extract_body_source(&chunk.text);
                            let span = Span::from_byte_range(
                                chunk.offset,
                                chunk.offset + chunk.text.len(),
                                src,
                            );
                            let tags = extract_doc_tags(src, chunk.offset);
                            tests.push(TestCase {
                                module: module.to_path_buf(),
                                name,
                                body_source,
                                span,
                                tags,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(tests)
}

/// Extract the expression text from a binding statement like
/// `test_foo :: expr` or a test declaration `test name = expr`.
///
/// Returns the text after the first binding separator — `::` (bind) or a
/// bare `=` (test-decl separator), whichever comes first — scanned outside
/// string literals and `--` comments (trimmed). If no separator is found,
/// returns the whole text.
fn extract_body_source(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];

        // String literal — skip to the closing quote (respecting escapes).
        if b == b'"' {
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => {
                        i += 2;
                        continue;
                    }
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }

        // Comment: `--` to end of line.
        if b == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // `::` binding separator (ADR-0079).
        if b == b':' && i + 1 < bytes.len() && bytes[i + 1] == b':' {
            return text[i + 2..].trim().to_string();
        }

        if b == b'=' {
            let prev = if i > 0 { bytes[i - 1] } else { 0 };
            let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            // Skip `==`, `>=`, `<=`, `!=`, `=>`
            if next == b'=' || prev == b'>' || prev == b'<' || prev == b'!' || next == b'>' {
                i += 1;
                continue;
            }
            // Found the `=` separator.
            return text[i + 1..].trim().to_string();
        }
        i += 1;
    }
    text.to_string()
}

/// Recursively collect all `.handl` and `.hnl` files from a directory.
fn collect_handl_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_handl_files(&path, files);
        } else if let Some(ext) = path.extension() {
            if ext == "hnl" || ext == "handl" {
                files.push(path);
            }
        }
    }
}

/// Discover tests in the given paths.
///
/// Each path is either a `.handl`/`.hnl` file or a directory to search
/// recursively. Tests are discovered by parsing each module and extracting
/// test declarations (bindings whose name starts with `test_`). Tests are
/// ordered by source position within each module, and modules are ordered
/// lexicographically by path.
///
/// # Errors
///
/// Returns [`DiscoverError`] if a path doesn't exist or a file can't be read.
/// Parse errors for individual modules are recorded in the returned
/// [`TestPlan::parse_errors`] rather than propagated.
pub fn discover(paths: &[PathBuf]) -> Result<TestPlan, DiscoverError> {
    crate::provider::discover_with(&crate::provider::FileSystemProvider, paths, None)
}

/// Push a [`TestEvent`] into `sink` when one is supplied.
///
/// Events are emitted only by the coordinator thread (discovery and the
/// run loop), never from scheduler workers, so event order is always
/// canonical and deterministic.
fn emit(sink: &mut Option<&mut Vec<TestEvent>>, event: TestEvent) {
    if let Some(events) = sink.as_deref_mut() {
        events.push(event);
    }
}

/// Filesystem discovery implementation behind
/// [`crate::provider::FileSystemProvider`].
///
/// Identical to [`discover`] plus an optional event sink: each discovered
/// test emits [`TestEvent::TestDiscovered`].
///
/// # Errors
///
/// See [`discover`].
pub(crate) fn discover_inner(
    paths: &[PathBuf],
    mut events: Option<&mut Vec<TestEvent>>,
) -> Result<TestPlan, DiscoverError> {
    let mut plan = TestPlan::default();

    // Validate paths and collect files.
    let mut files = Vec::new();
    for path in paths {
        if !path.exists() {
            return Err(DiscoverError::NotFound(path.display().to_string()));
        }
        if path.is_dir() {
            collect_handl_files(path, &mut files);
        } else if let Some(ext) = path.extension() {
            if ext == "hnl" || ext == "handl" {
                files.push(path.clone());
            }
        }
    }

    // Sort files lexicographically for deterministic ordering.
    files.sort();

    // Build the test language: canonical kit ⊕ test-language fragment
    // (ADR-0089 §"Test declaration syntax", ADR-0044).
    let lang = test_language().map_err(DiscoverError::Io)?;

    // Discover tests in each file.
    for file in &files {
        let src = std::fs::read_to_string(file)
            .map_err(|e| DiscoverError::Io(format!("{}: {e}", file.display())))?;

        if detect_dialect(&src) != Dialect::Canonical {
            continue;
        }

        match discover_tests_in_module(&lang, &src, file) {
            Ok(tests) => {
                for case in &tests {
                    emit(
                        &mut events,
                        TestEvent::TestDiscovered {
                            module: case.module.clone(),
                            name: case.name.clone(),
                        },
                    );
                }
                plan.tests.extend(tests);
            }
            Err(msg) => {
                plan.parse_errors.push((file.clone(), msg));
            }
        }
    }

    Ok(plan)
}

// ---------------------------------------------------------------------------
// Test execution
// ---------------------------------------------------------------------------

/// A [`ModuleLookup`] wrapper around the Kit's stdlib module chain.
///
/// The test runtime needs a stable `Rc<dyn ModuleLookup>` for the eval
/// engine; this wrapper preserves the optional delegate from
/// `kit.runtime().modules()` while remaining a concrete type.
struct TestModuleLookup {
    /// Delegate for all function lookups (stdlib, fp, meta, etc.).
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl ModuleLookup for TestModuleLookup {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        // Delegate to the Kit's module chain.
        self.delegate.as_ref().and_then(|d| d.lookup(name))
    }

    fn lookup_type_signature(&self, name: &str) -> Option<Value> {
        self.delegate
            .as_ref()
            .and_then(|d| d.lookup_type_signature(name))
    }
}

/// Build a test runtime: a [`Runtime`] with the canonical language, the
/// data lexer, Pratt parser, and tree-walk eval engine, wired with the
/// Kit's stdlib module chain and a keyword-assertion failure handler.
///
/// This gives test expressions access to the stdlib (`fn.*`, `math.*`,
/// `str.*`, etc.); `assert` is a keyword handled natively by the
/// tree-walk engine and is not a module lookup.
fn build_test_runtime() -> Result<Runtime, String> {
    let kit = handl_kit_canonical::Kit::new()
        .map_err(|e| format!("failed to assemble canonical kit: {e}"))?;
    // Canonical language ⊕ test-language fragment, so `test`-kind nodes
    // (whose eval scheme projects the `body` child) evaluate correctly.
    let lang = test_language()?;

    // Get the Kit's module chain (stdlib + fp + meta + hello).
    let kit_modules = kit.runtime().modules().cloned();

    // Wrap the Kit's module chain for the eval engine.
    let test_modules: Rc<dyn ModuleLookup> = Rc::new(TestModuleLookup {
        delegate: kit_modules,
    });

    // Build the eval engine with the test module chain + `self` global +
    // the keyword-assertion failure handler.
    let self_val = language_value(&lang);
    let eval_engine = TreeWalkEngine::new()
        .with_modules(test_modules)
        .with_global("self", self_val)
        .with_assert_handler(Rc::new(TestAssertHandler));

    // Build the runtime with lex, parse, and eval engines.
    let runtime = Runtime::new(lang)
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(eval_engine));

    Ok(runtime)
}

/// Run a test plan.
///
/// Thin wrapper over [`run_selected`]: selects all tests and runs them
/// sequentially. See [`crate::config::run_with_config`] for the full
/// planner/policy entry point.
pub fn run(plan: &TestPlan) -> RunReport {
    run_selected(plan, &Filter::all(), &SchedulePolicy::sequential())
}

/// Run a test plan with a filter pattern.
///
/// Thin wrapper over [`run_selected`]: the pattern becomes a
/// [`filter_predicate`] (substring or glob match on the test name; an
/// empty pattern selects all) and the run is sequential.
pub fn run_with_filter(plan: &TestPlan, filter: &str) -> RunReport {
    run_selected(
        plan,
        &filter_predicate(filter),
        &SchedulePolicy::sequential(),
    )
}

// ---------------------------------------------------------------------------
// Filtering (predicate model, ADR-0089)
// ---------------------------------------------------------------------------
//
// The filter itself is a **handl arrow** evaluated per test case against a
// record projection `{ name, path, tags }`. All logic — glob matching,
// substring search, tag-expression parsing — lives in
// `handl-packages/pred/pred.hnl` (the `pred` module replaces the deleted
// `plugin-lib-predicate` crate). Rust keeps only the spec record
// `{ names: [..], tags: [..] }` and the `pred.matches` host call.

/// The handl source for the `pred` predicate/filter library.
const PRED_HNL: &str = include_str!("../../../handl-packages/pred/pred.hnl");

/// The record field names exposed by `pred.hnl` — each becomes a callable
/// [`Function`] on the `pred` module.
const PRED_FUNCS: &[&str] = &[
    "always",
    "never",
    "and",
    "or",
    "not",
    "field",
    "exact",
    "glob",
    "contains",
    "match_glob",
    "build",
    "matches",
];

/// Resolver for `pred.hnl`'s `from "char" import` — maps `char` onto the
/// canonical native `char` backend (the same floor `str-handl` builds on).
struct PredResolver;

impl ModuleResolver for PredResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        match name {
            "char" => Some(Arc::new(CharModule)),
            _ => None,
        }
    }
}

static PRED_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// The shared `pred` [`HandlModule`], built lazily on first use.
fn pred_module() -> &'static HandlModule {
    PRED_MODULE
        .get_or_init(|| HandlModule::new("pred", PRED_HNL, PRED_FUNCS).with_resolver(&PredResolver))
}

/// A compiled case-selection filter — the host-side handle for the `pred`
/// module's spec record `{ names: [..], tags: [..] }`.
///
/// `names` is a list of name patterns (each is a glob on the test name when
/// it contains `*`/`?`, else a substring match); every pattern must match.
/// `tags` is a list of tag expressions (comma-separated OR terms, `!`
/// negation — see [`tag_expr_predicate`]); every expression must hold.
/// Semantics are defined entirely by `pred.hnl`; this type only composes
/// spec records and evaluates them via `pred.matches`.
#[derive(Debug, Clone)]
pub struct Filter {
    /// The `pred` spec record `{ names: [..], tags: [..] }`.
    spec: Value,
}

impl Filter {
    /// Select every test (the `pred.always` predicate).
    pub fn all() -> Self {
        Filter {
            spec: serde_json::json!({ "names": [], "tags": [] }),
        }
    }

    /// Compose with another filter: tests must satisfy both (logical AND).
    pub fn and(mut self, mut other: Filter) -> Filter {
        if let (Some(a), Some(b)) = (
            self.spec.get_mut("names").and_then(Value::as_array_mut),
            other.spec.get_mut("names").and_then(Value::as_array_mut),
        ) {
            a.append(b);
        }
        if let (Some(a), Some(b)) = (
            self.spec.get_mut("tags").and_then(Value::as_array_mut),
            other.spec.get_mut("tags").and_then(Value::as_array_mut),
        ) {
            a.append(b);
        }
        self
    }

    /// `true` when the spec selects everything (fast path — avoids spinning
    /// up the `pred` runtime for the common unfiltered run).
    fn selects_all(&self) -> bool {
        let empty = |k: &str| {
            self.spec
                .get(k)
                .and_then(Value::as_array)
                .map(|a| a.is_empty())
                .unwrap_or(true)
        };
        empty("names") && empty("tags")
    }

    /// Evaluate the filter against `case` by projecting it to the record
    /// `{ name, path, tags }` and applying the compiled `pred` filter arrow
    /// (`pred.matches`) in a single in-engine evaluation.
    pub fn eval(&self, case: &TestCase) -> bool {
        if self.selects_all() {
            return true;
        }
        let case_record = serde_json::json!({
            "name": case.name,
            "path": case.module.display().to_string(),
            "tags": case.tags,
        });
        let mut ctx = EvalContext::new();
        pred_module()
            .function("matches")
            .map(|f| f.call(&[self.spec.clone(), case_record], &mut ctx))
            .and_then(|r| r.ok())
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter::all()
    }
}

/// Build a [`Filter`] from a filter pattern string.
///
/// An empty pattern selects all tests ([`Filter::all`]). A pattern
/// containing glob metacharacters (`*` or `?`) is matched against the test
/// **name** as a glob; a plain pattern is a substring match on the name,
/// mirroring [`run_with_filter`]. The match itself is evaluated in handl
/// by `pred.hnl`.
///
/// Composable via [`Filter::and`] (ADR-0089 §"Filtering as a predicate
/// model").
pub fn filter_predicate(pattern: &str) -> Filter {
    if pattern.is_empty() {
        return Filter::all();
    }
    Filter {
        spec: serde_json::json!({ "names": [pattern], "tags": [] }),
    }
}

/// Build a [`Filter`] that selects tests tagged with `tag`.
///
/// Tags are discovered from `-- tags: ...` doc-comment lines above a test
/// declaration. An empty `tag` selects all tests.
pub fn tag_predicate(tag: &str) -> Filter {
    if tag.is_empty() {
        return Filter::all();
    }
    Filter {
        spec: serde_json::json!({ "names": [], "tags": [tag] }),
    }
}

/// Build a [`Filter`] from a tag expression.
///
/// Grammar (per `--tag` flag value):
///
/// ```text
/// expr := term ("," term)*     — comma separates alternatives (OR)
/// term := "!"? name            — `!` negates the tag (NOT)
/// ```
///
/// So `"slow,net"` selects tests tagged `slow` OR `net`, `"!slow"` selects
/// tests NOT tagged `slow`, and `"a,!b"` selects tests tagged `a` OR not
/// tagged `b`. Repeated `--tag` flags are combined with AND via
/// [`Filter::and`]. An empty expression (or one with only empty terms)
/// selects all tests.
pub fn tag_expr_predicate(expr: &str) -> Filter {
    let any = expr.split(',').any(|term| !term.trim().is_empty());
    if !any {
        return Filter::all();
    }
    Filter {
        spec: serde_json::json!({ "names": [], "tags": [expr] }),
    }
}

// ---------------------------------------------------------------------------
// Scheduled execution (generic bounded-job scheduler, ADR-0089)
// ---------------------------------------------------------------------------

thread_local! {
    /// Per-worker cached test runtime.
    ///
    /// [`Runtime`] is not `Send` (the module chain is `Rc`-based), so it
    /// cannot be shared across scheduler worker threads. Each worker builds
    /// one lazily on first use and reuses it across all jobs dispatched to
    /// that thread.
    static TEST_RUNTIME: RefCell<Option<Rc<Runtime>>> = RefCell::new(None);
}

/// Whether a test-body value is an `err`-tagged result (the `assert`
/// keyword returns `result.err(...)` on failure; ADR-0077, ADR-0089).
fn is_err_result(value: &Value) -> bool {
    as_tag(value) == Some("err")
}

/// Extract the human-readable message from an `err`-tagged result value.
fn extract_err_message(value: &Value) -> Option<String> {
    let payload = err_payload(value)?;
    payload
        .get("message")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Extract a [`FailureDetail`] from an `err`-tagged result value as a
/// fallback when no side-channel failure detail was recorded.
fn failure_detail_from_value(value: &Value) -> FailureDetail {
    if let Some(payload) = err_payload(value) {
        FailureDetail {
            assertion: payload
                .get("assertion")
                .and_then(|v| v.as_str())
                .map(String::from),
            expected: payload
                .get("expected")
                .and_then(|v| v.as_str())
                .map(String::from),
            actual: payload
                .get("actual")
                .and_then(|v| v.as_str())
                .map(String::from),
            diff: payload
                .get("diff")
                .and_then(|v| v.as_str())
                .map(String::from),
            cause: None,
        }
    } else {
        FailureDetail::default()
    }
}

/// Return the structured payload object inside an `err`-tagged result.
///
/// For `result.err({...})` this is `value["value"]`; if the value is a
/// tagged object with fields merged directly it is the object itself.
fn err_payload<'v>(value: &'v Value) -> Option<&'v serde_json::Map<String, Value>> {
    let obj = value.as_object()?;
    if obj.contains_key(handl_language::TAG_VALUE_KEY) {
        obj.get("value").and_then(|v| v.as_object())
    } else {
        Some(obj)
    }
}

/// Evaluate a single test body on the calling thread's cached runtime.
///
/// Returns the [`TestResult`], measured duration, and any structured
/// failure detail captured from the assert framework. The `Instant` is
/// taken inside the job closure (i.e. on the worker thread), so the
/// measurement is parallel-safe and independent of dispatch order.
fn eval_test_body(body_source: &str) -> (TestResult, Duration, FailureDetail) {
    crate::framework::clear_last_assert_failure();
    let start = Instant::now();
    let (result, failure) = TEST_RUNTIME.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if borrow.is_none() {
            match build_test_runtime() {
                Ok(rt) => *borrow = Some(Rc::new(rt)),
                Err(msg) => {
                    return (
                        TestResult::Fail {
                            message: format!("runtime setup error: {msg}"),
                        },
                        FailureDetail::default(),
                    );
                }
            }
        }
        let rt = borrow.as_ref().expect("runtime initialized above");
        match rt.run(&Source::new(body_source)) {
            Ok(value) => {
                if is_err_result(&value) {
                    let failure = crate::framework::take_last_assert_failure()
                        .unwrap_or_else(|| failure_detail_from_value(&value));
                    let message = extract_err_message(&value)
                        .unwrap_or_else(|| "assertion failed".to_string());
                    (TestResult::Fail { message }, failure)
                } else {
                    (TestResult::Pass, FailureDetail::default())
                }
            }
            Err(e) => {
                let failure = crate::framework::take_last_assert_failure().unwrap_or_default();
                (
                    TestResult::Fail {
                        message: e.to_string(),
                    },
                    failure,
                )
            }
        }
    });
    (result, start.elapsed(), failure)
}

/// Convert a scheduler [`JobError`] into a run result.
fn job_error_result(err: JobError) -> (TestResult, Duration, FailureDetail) {
    match err {
        JobError::Panic => (
            TestResult::Fail {
                message: "test job panicked".to_string(),
            },
            Duration::ZERO,
            FailureDetail::default(),
        ),
        JobError::Timeout => (
            TestResult::Fail {
                message: "test job timed out".to_string(),
            },
            Duration::ZERO,
            FailureDetail::default(),
        ),
    }
}

/// Run a test plan, selecting tests with a [`Filter`] and executing them
/// under a [`SchedulePolicy`] via the generic [`BoundedScheduler`]
/// (ADR-0089 §"Planner + Scheduler").
///
/// Tests the filter rejects are reported as [`TestResult::Skip`]. Selected
/// tests are dispatched to the scheduler as independent jobs keyed by plan
/// index; results are re-collected into canonical plan order, so parallel
/// execution changes timing but never logical result ordering (ADR-0089
/// §"Determinism").
///
/// Each worker thread reuses a lazily built [`Runtime`] (see
/// [`TEST_RUNTIME`]); there is no shared state between tests.
pub fn run_selected(plan: &TestPlan, predicate: &Filter, policy: &SchedulePolicy) -> RunReport {
    run_selected_impl(plan, predicate, policy, None, None)
}

/// Shared implementation for [`run_selected`], [`run_selected_cached`],
/// and [`crate::config::run_with_config`].
///
/// When `events` is supplied, the coordinator emits the ADR-0089 event
/// vocabulary: `RunStarted`, then per module `SuiteStarted`, the module's
/// test events in canonical plan order (`TestSelected`/`TestSkipped`/
/// `TestCached`/`TestStarted`/`TestFinished` + `TestFailed`),
/// `SuiteFinished`, and `RunFinished` last. Events are emitted only on
/// this thread, so ordering is deterministic under any `--jobs`.
pub(crate) fn run_selected_impl(
    plan: &TestPlan,
    predicate: &Filter,
    policy: &SchedulePolicy,
    mut cache: Option<&mut TestRunCache>,
    mut events: Option<&mut Vec<TestEvent>>,
) -> RunReport {
    let run_start = Instant::now();
    emit(
        &mut events,
        TestEvent::RunStarted {
            total: plan.tests.len(),
        },
    );

    // Selection / cache lookup pass: decide, for each plan index, whether
    // it is skipped, served from cache, or must be executed. Build the
    // scheduler jobs for the tests that need to run.
    let mut states: Vec<TestState> = Vec::with_capacity(plan.tests.len());
    let mut jobs: Vec<(
        usize,
        Box<dyn FnOnce() -> (TestResult, Duration, FailureDetail) + Send>,
    )> = Vec::new();

    for (i, case) in plan.tests.iter().enumerate() {
        if !predicate.eval(case) {
            states.push(TestState::Skip);
            continue;
        }

        if let Some(c) = cache.as_deref_mut() {
            if let Some((cached, failure)) = c.lookup(case) {
                c.record_hit();
                states.push(TestState::Cached(cached, failure));
                continue;
            }
            c.record_miss();
        }

        states.push(TestState::Run);
        let body = case.body_source.clone();
        jobs.push((i, Box::new(move || eval_test_body(&body))));
    }

    // Plan tests are already sorted by module. Capture each module's range
    // so suite events can bracket the module's test events.
    let mut module_ranges: Vec<(PathBuf, usize, usize)> = Vec::new();
    let mut i = 0;
    while i < plan.tests.len() {
        let module = plan.tests[i].module.clone();
        let start = i;
        i += 1;
        while i < plan.tests.len() && plan.tests[i].module == module {
            i += 1;
        }
        module_ranges.push((module, start, i));
    }

    // Dispatch uncached, selected tests to the generic bounded scheduler.
    let mut results: Vec<Option<(TestResult, Duration, FailureDetail)>> =
        vec![None; plan.tests.len()];
    for (i, job_result) in BoundedScheduler::run(jobs, policy) {
        let (result, duration, failure) = match job_result.output {
            Ok(triple) => triple,
            Err(e) => job_error_result(e),
        };
        if let Some(c) = cache.as_deref_mut() {
            c.store(&plan.tests[i], &result, &failure);
        }
        results[i] = Some((result, duration, failure));
    }

    // Emit events and build the report in canonical plan order, one module
    // at a time, so `SuiteStarted`/`SuiteFinished` bracket each module's
    // test events.
    let mut report = RunReport::default();
    for (module, start, end) in &module_ranges {
        emit(
            &mut events,
            TestEvent::SuiteStarted {
                name: module.display().to_string(),
            },
        );
        for i in *start..*end {
            let case = &plan.tests[i];
            match &states[i] {
                TestState::Skip => {
                    emit(
                        &mut events,
                        TestEvent::TestSkipped {
                            module: case.module.clone(),
                            name: case.name.clone(),
                        },
                    );
                    report.outcomes.push(TestOutcome {
                        case: case.clone(),
                        result: TestResult::Skip,
                        duration: Duration::ZERO,
                        failure: FailureDetail::default(),
                    });
                }
                TestState::Cached(result, failure) => {
                    emit(
                        &mut events,
                        TestEvent::TestSelected {
                            module: case.module.clone(),
                            name: case.name.clone(),
                        },
                    );
                    emit(
                        &mut events,
                        TestEvent::TestCached {
                            module: case.module.clone(),
                            name: case.name.clone(),
                        },
                    );
                    report.outcomes.push(TestOutcome {
                        case: case.clone(),
                        result: result.clone(),
                        duration: Duration::ZERO,
                        failure: failure.clone(),
                    });
                }
                TestState::Run => {
                    emit(
                        &mut events,
                        TestEvent::TestSelected {
                            module: case.module.clone(),
                            name: case.name.clone(),
                        },
                    );
                    emit(
                        &mut events,
                        TestEvent::TestStarted {
                            module: case.module.clone(),
                            name: case.name.clone(),
                        },
                    );

                    let (result, duration, failure) = results[i].take().unwrap_or_else(|| {
                        (
                            TestResult::Fail {
                                message: "internal error: test result not collected".to_string(),
                            },
                            Duration::ZERO,
                            FailureDetail::default(),
                        )
                    });

                    emit(
                        &mut events,
                        TestEvent::TestFinished {
                            module: case.module.clone(),
                            name: case.name.clone(),
                            result: result.clone(),
                            duration,
                        },
                    );
                    if let TestResult::Fail { message } = &result {
                        emit(
                            &mut events,
                            TestEvent::TestFailed {
                                module: case.module.clone(),
                                name: case.name.clone(),
                                message: message.clone(),
                            },
                        );
                    }
                    report.outcomes.push(TestOutcome {
                        case: case.clone(),
                        result,
                        duration,
                        failure,
                    });
                }
            }
        }
        emit(
            &mut events,
            TestEvent::SuiteFinished {
                name: module.display().to_string(),
            },
        );
    }

    emit(
        &mut events,
        TestEvent::RunFinished {
            passed: report.passed(),
            failed: report.failed(),
            skipped: report.skipped(),
            duration: run_start.elapsed(),
        },
    );
    report
}

// ---------------------------------------------------------------------------
// Test-result cache (opt-in `--cache`, ADR-0089 §"Cache domains")
// ---------------------------------------------------------------------------

/// Opt-in, in-memory and disk-persisted test-result cache
/// (`ExecutionCache` in ADR-0089).
///
/// The generic eval-result memoisation concern lives in
/// `plugin-exec-cache`; this is the thin **test-domain layer** over it. Keys
/// reuse the generic fingerprint machinery — an [`AstKey`] whose `kind` is
/// `"test.result"` and whose `payload_json` serializes the test identity
/// (`module` path + `name` + `body_source`), so a test is re-evaluated when
/// its source changes. Values are recorded `(TestResult, FailureDetail)` pairs.
///
/// For pure tests `same test + same source + same runtime = same result`,
/// so a cached second run replays `Pass`/`Fail` outcomes without
/// re-evaluating — identical in ordering and content to a fresh run.
///
/// The handle is caller-provided and reusable across runs; it is **not**
/// used unless passed to [`run_selected_cached`]. Result caching is opt-in
/// per the ADR-0089 `--cache` flag. Disk persistence is provided by
/// [`TestRunCache::load`] and [`TestRunCache::save`].
#[derive(Debug, Default)]
pub struct TestRunCache {
    /// Recorded results keyed by test identity fingerprint.
    entries: HashMap<AstKey, (TestResult, FailureDetail)>,
    /// Number of lookups served from the cache.
    hits: usize,
    /// Number of lookups that had to execute.
    misses: usize,
}

/// On-disk cache file layout.
///
/// Versioned so future changes can reject stale caches.
#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    entries: Vec<CacheEntry>,
}

/// A single persisted cache entry.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    kind: String,
    payload_json: String,
    result: TestResult,
    failure: FailureDetail,
}

impl TestRunCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of cache hits recorded since creation (or [`Self::clear`]).
    pub fn hits(&self) -> usize {
        self.hits
    }

    /// The number of cache misses recorded since creation (or [`Self::clear`]).
    pub fn misses(&self) -> usize {
        self.misses
    }

    /// Load a cache from a JSON file.
    ///
    /// Returns an error if the file cannot be read or parsed, or if the
    /// on-disk version is not supported. Callers typically fall back to an
    /// empty cache.
    pub fn load<P: AsRef<std::path::Path>>(path: P) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let file: CacheFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if file.version != 1 {
            return Err("unsupported cache version".to_string());
        }
        let mut entries = HashMap::new();
        for entry in file.entries {
            entries.insert(
                AstKey {
                    kind: entry.kind,
                    payload_json: entry.payload_json,
                },
                (entry.result, entry.failure),
            );
        }
        Ok(Self {
            entries,
            hits: 0,
            misses: 0,
        })
    }

    /// Save the cache to a JSON file, creating parent directories as needed.
    pub fn save<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), String> {
        let entries: Vec<CacheEntry> = self
            .entries
            .iter()
            .map(|(key, (result, failure))| CacheEntry {
                kind: key.kind.clone(),
                payload_json: key.payload_json.clone(),
                result: result.clone(),
                failure: failure.clone(),
            })
            .collect();
        let file = CacheFile {
            version: 1,
            entries,
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Clear all entries and statistics.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// The cache key for a test case: `AstKey` fingerprint of
    /// `{module, name, body_source}` under the `"test.result"` kind.
    fn key_for(case: &TestCase) -> AstKey {
        AstKey {
            kind: "test.result".to_string(),
            payload_json: serde_json::json!({
                "module": case.module.display().to_string(),
                "name": &case.name,
                "body_source": &case.body_source,
            })
            .to_string(),
        }
    }

    /// Look up the recorded result for a test case.
    fn lookup(&self, case: &TestCase) -> Option<(TestResult, FailureDetail)> {
        self.entries.get(&Self::key_for(case)).cloned()
    }

    /// Record a freshly computed result for a test case.
    fn store(&mut self, case: &TestCase, result: &TestResult, failure: &FailureDetail) {
        self.entries
            .insert(Self::key_for(case), (result.clone(), failure.clone()));
    }

    /// Record a cache hit (stats only).
    fn record_hit(&mut self) {
        self.hits += 1;
    }

    /// Record a cache miss (stats only).
    fn record_miss(&mut self) {
        self.misses += 1;
    }
}

/// Run a test plan like [`run_selected`], but consult and populate the
/// caller-provided [`TestRunCache`] (the ADR-0089 `--cache` flag made
/// programmatic).
///
/// Selected tests with a cached result replay it without re-evaluating;
/// uncached tests execute under `policy` and their `Pass`/`Fail` results
/// are stored. Cached reports are identical in ordering and content to a
/// fresh run.
pub fn run_selected_cached(
    plan: &TestPlan,
    predicate: &Filter,
    policy: &SchedulePolicy,
    cache: &mut TestRunCache,
) -> RunReport {
    run_selected_impl(plan, predicate, policy, Some(cache), None)
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

/// Format a run report as human-readable output.
///
/// One line per test (`PASS`/`FAIL`/`SKIP`), summary counts, failures with
/// error messages and locations.
///
/// Thin wrapper over the reporting pipeline: `summarize` →
/// [`crate::report::HumanReporter`] (ADR-0089 §"Reporting pipeline").
pub fn format_report_human(report: &RunReport) -> String {
    use crate::report::{HumanReporter, Reporter, summarize};
    HumanReporter.format(report, &summarize(report))
}

/// Format a run report as JSON for CI consumers.
///
/// Thin wrapper over the reporting pipeline: `summarize` →
/// [`crate::report::JsonReporter`].
pub fn format_report_json(report: &RunReport) -> String {
    use crate::report::{JsonReporter, Reporter, summarize};
    JsonReporter.format(report, &summarize(report))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_default_is_empty() {
        let plan = TestPlan::default();
        assert!(plan.tests.is_empty());
        assert!(plan.parse_errors.is_empty());
    }

    #[test]
    fn run_report_default_has_zero_counts() {
        let report = RunReport::default();
        assert_eq!(report.passed(), 0);
        assert_eq!(report.failed(), 0);
        assert_eq!(report.skipped(), 0);
        assert_eq!(report.total(), 0);
        assert!(!report.has_failures());
    }

    #[test]
    fn discover_error_display() {
        let io_err = DiscoverError::Io("disk full".to_string());
        assert_eq!(format!("{io_err}"), "io error: disk full");

        let not_found = DiscoverError::NotFound("/missing".to_string());
        assert_eq!(format!("{not_found}"), "path not found: /missing");
    }

    #[test]
    fn discover_error_equality() {
        assert_eq!(
            DiscoverError::Io("x".to_string()),
            DiscoverError::Io("x".to_string())
        );
        assert_ne!(
            DiscoverError::Io("x".to_string()),
            DiscoverError::Io("y".to_string())
        );
        assert_eq!(
            DiscoverError::NotFound("x".to_string()),
            DiscoverError::NotFound("x".to_string())
        );
    }

    #[test]
    fn test_result_variants() {
        assert_eq!(TestResult::Pass, TestResult::Pass);
        assert_eq!(
            TestResult::Fail {
                message: "boom".to_string()
            },
            TestResult::Fail {
                message: "boom".to_string()
            }
        );
        assert_ne!(
            TestResult::Fail {
                message: "boom".to_string()
            },
            TestResult::Fail {
                message: "other".to_string()
            }
        );
        assert_eq!(TestResult::Skip, TestResult::Skip);
    }

    #[test]
    fn run_report_counts() {
        let report = RunReport {
            outcomes: vec![
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("a.hnl"),
                        name: "test_a".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Pass,
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("a.hnl"),
                        name: "test_b".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Fail {
                        message: "oops".to_string(),
                    },
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("a.hnl"),
                        name: "test_c".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Skip,
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
            ],
        };
        assert_eq!(report.total(), 3);
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 1);
        assert_eq!(report.skipped(), 1);
        assert!(report.has_failures());
    }

    #[test]
    fn split_statements_basic() {
        let chunks = split_statements("a = 1\nb = 2\nc = 3");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "a = 1");
        assert_eq!(chunks[1].text, "b = 2");
        assert_eq!(chunks[2].text, "c = 3");
    }

    #[test]
    fn split_statements_semicolons() {
        let chunks = split_statements("a = 1; b = 2; c = 3");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "a = 1");
        assert_eq!(chunks[1].text, "b = 2");
        assert_eq!(chunks[2].text, "c = 3");
    }

    #[test]
    fn split_statements_respects_brackets() {
        let chunks = split_statements("a = [1, 2\n, 3]\nb = 2");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "a = [1, 2\n, 3]");
        assert_eq!(chunks[1].text, "b = 2");
    }

    #[test]
    fn split_statements_respects_strings() {
        let chunks = split_statements("a = \"hello\nworld\"\nb = 2");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "a = \"hello\nworld\"");
        assert_eq!(chunks[1].text, "b = 2");
    }

    #[test]
    fn split_statements_ignores_comments() {
        let chunks = split_statements("-- a comment\na = 1\nb = 2");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "a = 1");
        assert_eq!(chunks[1].text, "b = 2");
    }

    #[test]
    fn split_statements_empty() {
        let chunks = split_statements("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn extract_body_source_simple() {
        assert_eq!(extract_body_source("test_foo = 42"), "42");
        assert_eq!(extract_body_source("test_bar = 1 + 1"), "1 + 1");
    }

    #[test]
    fn extract_body_source_no_equals() {
        assert_eq!(extract_body_source("42"), "42");
    }

    #[test]
    fn extract_body_source_skips_comparison() {
        // `==` should not be treated as a binding `=`.
        assert_eq!(extract_body_source("test_eq = 1 == 1"), "1 == 1");
    }

    #[test]
    fn extract_body_source_bind_operator() {
        // `::` is the binding separator (ADR-0079); `=` inside the body is
        // equality, not the separator. Regression: previously the first `=`
        // was used, which truncated `assert a = b` bodies to just the
        // expected literal — so every `assert` test passed vacuously.
        assert_eq!(
            extract_body_source("test_foo :: assert 1 = 2"),
            "assert 1 = 2"
        );
    }

    #[test]
    fn extract_body_source_skips_equals_in_strings() {
        // `=` inside a string literal is not the separator.
        assert_eq!(
            extract_body_source("test_foo :: \"x = y\" >>> f = \"z\""),
            "\"x = y\" >>> f = \"z\""
        );
        // And `::` inside a string literal is not the separator either.
        assert_eq!(extract_body_source("test_foo = \"x :: y\""), "\"x :: y\"");
    }

    #[test]
    fn eval_test_body_fails_on_false_equality() {
        // Regression: `assert 1 = 2` must fail, not silently pass.
        let (result, _dur, _detail) = eval_test_body("assert 1 = 2");
        assert!(
            matches!(result, TestResult::Fail { .. }),
            "expected `assert 1 = 2` to fail, got {result:?}"
        );
    }

    #[test]
    fn eval_test_body_passes_on_true_equality() {
        let (result, _dur, _detail) = eval_test_body("assert 1 = 1");
        assert!(
            matches!(result, TestResult::Pass),
            "expected `assert 1 = 1` to pass, got {result:?}"
        );
    }

    #[test]
    fn format_report_human_basic() {
        let report = RunReport {
            outcomes: vec![
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_a".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Pass,
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_b".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Fail {
                        message: "expected true".to_string(),
                    },
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_c".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Skip,
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
            ],
        };
        let out = format_report_human(&report);
        assert!(out.contains("PASS  test_a (mod.hnl)"));
        assert!(out.contains("FAIL  test_b (mod.hnl): expected true"));
        assert!(out.contains("SKIP  test_c (mod.hnl)"));
        assert!(out.contains("3 tests: 1 passed, 1 failed, 1 skipped"));
    }

    #[test]
    fn format_report_json_valid() {
        let report = RunReport {
            outcomes: vec![
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_a".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Pass,
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_b".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                        tags: Vec::new(),
                    },
                    result: TestResult::Fail {
                        message: "oops".to_string(),
                    },
                    duration: Duration::ZERO,
                    failure: FailureDetail::default(),
                },
            ],
        };
        let json = format_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed["tests"].is_array());
        let tests = parsed["tests"].as_array().unwrap();
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0]["status"], "pass");
        assert_eq!(tests[1]["status"], "fail");
        assert_eq!(tests[1]["message"], "oops");
        assert_eq!(parsed["summary"]["total"], 2);
        assert_eq!(parsed["summary"]["passed"], 1);
        assert_eq!(parsed["summary"]["failed"], 1);
        assert_eq!(parsed["summary"]["skipped"], 0);
    }

    #[test]
    fn format_report_json_empty() {
        let report = RunReport::default();
        let json = format_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed["tests"].as_array().unwrap().is_empty());
        assert_eq!(parsed["summary"]["total"], 0);
    }

    #[test]
    fn extract_doc_tags_basic() {
        let src = r#"-- tags: slow
-- tags: net, flaky
test tagged_slow = 1 + 1
test tagged_fast = 1 + 1
"#;
        let offset = src.find("test tagged_slow").unwrap();
        let tags = super::extract_doc_tags(src, offset);
        assert_eq!(tags, vec!["slow", "net", "flaky"]);

        let offset_fast = src.find("test tagged_fast").unwrap();
        let tags_fast = super::extract_doc_tags(src, offset_fast);
        assert!(tags_fast.is_empty());
    }

    // --- tag expressions (`--tag` values: OR via `,`, NOT via `!`) ---------

    fn tagged_case(tags: &[&str]) -> TestCase {
        TestCase {
            module: PathBuf::from("m.hnl"),
            name: "t".to_string(),
            body_source: "1".to_string(),
            span: Span::default(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    // --- `pred` handl module (glob parity with the old Rust impl) --------

    /// Evaluate `pred.match_glob(pat, s)` in the handl module.
    fn match_glob(pat: &str, s: &str) -> bool {
        let mut ctx = EvalContext::new();
        super::pred_module()
            .function("match_glob")
            .unwrap()
            .call(&[serde_json::json!(pat), serde_json::json!(s)], &mut ctx)
            .unwrap()
            .as_bool()
            .unwrap()
    }

    #[test]
    fn match_glob_semantics() {
        // Parity with the deleted `plugin-lib-predicate` glob tests.
        assert!(match_glob("*.hnl", "foo.hnl"));
        assert!(!match_glob("*.hnl", "foo.txt"));
        assert!(match_glob("test_*", "test_foo"));
        assert!(match_glob("a?c", "abc"));
        assert!(!match_glob("a?c", "abbc"));
        // `*` matches the empty sequence; `**` collapses to `*`.
        assert!(match_glob("a*", "a"));
        // `**` collapses to `*` (flat matching): `**/*.hnl` still needs `/`.
        assert!(!match_glob("**/*.hnl", "foo.hnl"));
        assert!(match_glob("**/*.hnl", "a/b.hnl"));
        assert!(match_glob("*a*b*", "xxaxxbyy"));
        assert!(!match_glob("*a*b*", "xxbxx"));
        // literal chars and `?` boundaries
        assert!(match_glob("", ""));
        assert!(!match_glob("", "a"));
        assert!(!match_glob("?", ""));
        assert!(match_glob("?", "a"));
        assert!(match_glob("*?", "a"));
    }

    #[test]
    fn filter_eval_via_pred_module() {
        // Substring and glob name filters evaluate through `pred.matches`.
        let case = tagged_case(&[]);
        assert!(super::filter_predicate("t").eval(&case));
        assert!(!super::filter_predicate("zzz").eval(&case));
        assert!(super::filter_predicate("t*").eval(&case));
        assert!(!super::filter_predicate("x*").eval(&case));
    }

    #[test]
    fn tag_expr_single() {
        let p = super::tag_expr_predicate("slow");
        assert!(p.eval(&tagged_case(&["slow"])));
        assert!(!p.eval(&tagged_case(&["net"])));
        assert!(!p.eval(&tagged_case(&[])));
    }

    #[test]
    fn tag_expr_or() {
        // `slow,net` selects tests tagged slow OR net.
        let p = super::tag_expr_predicate("slow,net");
        assert!(p.eval(&tagged_case(&["slow"])));
        assert!(p.eval(&tagged_case(&["net"])));
        assert!(p.eval(&tagged_case(&["slow", "net"])));
        assert!(!p.eval(&tagged_case(&["flaky"])));
        assert!(!p.eval(&tagged_case(&[])));
    }

    #[test]
    fn tag_expr_negation() {
        // `!slow` excludes tests tagged slow.
        let p = super::tag_expr_predicate("!slow");
        assert!(!p.eval(&tagged_case(&["slow"])));
        assert!(p.eval(&tagged_case(&["net"])));
        assert!(p.eval(&tagged_case(&[])));
    }

    #[test]
    fn tag_expr_or_with_negation() {
        // `a,!b` selects tests tagged a OR not tagged b.
        let p = super::tag_expr_predicate("a,!b");
        assert!(p.eval(&tagged_case(&["a", "b"])));
        assert!(p.eval(&tagged_case(&["c"])));
        assert!(!p.eval(&tagged_case(&["b"])));
    }

    #[test]
    fn tag_expr_and_across_flags() {
        // Repeated `--tag` flags AND together: `a` AND NOT `b`.
        let p = super::tag_expr_predicate("a").and(super::tag_expr_predicate("!b"));
        assert!(p.eval(&tagged_case(&["a"])));
        assert!(p.eval(&tagged_case(&["a", "c"])));
        assert!(!p.eval(&tagged_case(&["a", "b"])));
        assert!(!p.eval(&tagged_case(&["b"])));
        assert!(!p.eval(&tagged_case(&[])));
    }

    #[test]
    fn tag_expr_empty_selects_all() {
        let p = super::tag_expr_predicate("");
        assert!(p.eval(&tagged_case(&["slow"])));
        assert!(p.eval(&tagged_case(&[])));
        // Whitespace-only terms are ignored.
        let p = super::tag_expr_predicate(" , ");
        assert!(p.eval(&tagged_case(&[])));
    }
}
