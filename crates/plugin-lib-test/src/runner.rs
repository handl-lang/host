//! Bootstrap test runner (ADR-0089).
//!
//! Discovers, runs, and reports tests written in handl. The runner parses
//! modules with the canonical kit's Pratt parser, extracts test declarations
//! (bindings whose name starts with `test_`), and evaluates each test body
//! with the tree-walk eval engine. Failures surface as raised errors, which
//! the runner catches and records.
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
//! test_failure = assert.true(false)
//! ```
//!
//! The first test passes (the expression evaluates without error); the second
//! fails (the assertion raises an error). If the grammar later gains a `test`
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

use std::path::{Path, PathBuf};
use std::rc::Rc;

use handl_language::{language_value, Ast, AstNode, AstRef, Language, Source, Value};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse::ParserEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_ext_diag::Span;
use handl_runtime::Runtime;
use handl_platform::{Function, Module, ModuleLookup};

use crate::assert::AssertModule;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// A single test's outcome with metadata.
#[derive(Debug, Clone)]
pub struct TestOutcome {
    /// The test case.
    pub case: TestCase,
    /// The result.
    pub result: TestResult,
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

// ---------------------------------------------------------------------------
// Source-chunk splitting
// ---------------------------------------------------------------------------

/// A top-level statement chunk: its trimmed text and byte offset in the
/// original source.
///
/// The v0.1 Pratt parser parses a single expression per call, so the runner
/// splits the source into statements at bracket-depth zero, respecting string
/// literals and `--` comments. This mirrors the approach used by the
/// bootstrap typechecker (ADR-0088).
struct Chunk {
    /// The trimmed statement text.
    text: String,
    /// Byte offset of the trimmed text's first byte in the original source.
    offset: usize,
}

/// Split `src` into top-level statement chunks.
///
/// The v0.1 surface separates top-level declarations by newlines or
/// semicolons. Splits at bracket-depth zero, respecting string literals and
/// `--` comments.
fn split_statements(src: &str) -> Vec<Chunk> {
    let bytes = src.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    let mut i = 0usize;

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

        // Comment: `--` (or `---`) to end of line. Skip the comment text
        // and advance `start` past it so it doesn't become part of the
        // next chunk.
        if b == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            // If we stopped at a newline, let the separator handler below
            // push the preceding chunk (which will be empty/comment-only
            // and trimmed away). Advance `start` to skip the comment.
            start = i;
            continue;
        }

        // Bracket nesting.
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }

        // Statement separators at depth zero.
        if depth == 0 && (b == b'\n' || b == b';') {
            push_chunk(src, &mut chunks, start, i);
            i += 1;
            start = i;
            continue;
        }

        i += 1;
    }

    // Trailing chunk.
    push_chunk(src, &mut chunks, start, src.len());
    chunks
}

/// Push a trimmed chunk from `src[start..end]` if it is non-empty and
/// not an import declaration (`from ... import ...` / `import ...`).
fn push_chunk(src: &str, chunks: &mut Vec<Chunk>, start: usize, end: usize) {
    let slice = &src[start..end];
    let lead = slice.len() - slice.trim_start().len();
    let trimmed = slice.trim();
    if trimmed.is_empty() {
        return;
    }
    // Skip import declarations — they are not valid bare expressions.
    // The test runtime already registers stdlib modules (str, fn, dedent,
    // etc.) via the kit's module chain, so test files don't need imports.
    if trimmed.starts_with("from ")
        || trimmed.starts_with("from\t")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("import\t")
        || trimmed.starts_with("import\"")
        || trimmed.starts_with("import'")
    {
        return;
    }
    chunks.push(Chunk {
        text: trimmed.to_string(),
        offset: start + lead,
    });
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

/// The name carried by a `var` node (its payload is the identifier string).
/// Returns `None` for non-`var` nodes.
fn var_name(node: &AstNode) -> Option<String> {
    if node.kind == "var" {
        node.payload.as_str().map(String::from)
    } else {
        None
    }
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

        // Check for a `test` node kind (future grammar support).
        let is_test_kind = root.kind.contains("test");

        // Binding: a node with `target` + `value` roles (data-driven —
        // any shape declaring these roles is a binding, not just `assign`).
        let (target_idx, value_idx) = binding_roles(lang, &root.kind);
        if let (Some(ti), Some(vi)) = (target_idx, value_idx) {
            let children = children_of(root);
            if let (Some(&target_raw), Some(&value_raw)) =
                (children.get(ti), children.get(vi))
            {
                if let (Ok(target_node), Ok(_value_node)) = (
                    ast.at(AstRef::new(target_raw)),
                    ast.at(AstRef::new(value_raw)),
                ) {
                    if let Some(name) = var_name(target_node) {
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
                            tests.push(TestCase {
                                module: module.to_path_buf(),
                                name,
                                body_source,
                                span,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(tests)
}

/// Extract the expression text from a binding statement like `test_foo = expr`.
///
/// Returns the text after the first `=` sign (trimmed). If no `=` is found,
/// returns the whole text.
fn extract_body_source(text: &str) -> String {
    // Find the first `=` that is not `==`, `>=`, `<=`, `!=`, `=>`.
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            let prev = if i > 0 { bytes[i - 1] } else { 0 };
            let next = if i + 1 < bytes.len() {
                bytes[i + 1]
            } else {
                0
            };
            // Skip `==`, `>=`, `<=`, `!=`, `=>`
            if next == b'=' || prev == b'>' || prev == b'<' || prev == b'!' || next == b'>' {
                i += 1;
                continue;
            }
            // Found the binding `=`.
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

    // Build the canonical language for parsing.
    let kit = handl_kit_canonical::Kit::new().map_err(|e| {
        DiscoverError::Io(format!("failed to assemble canonical kit: {e}"))
    })?;
    let lang = kit.runtime().lang().clone();

    // Discover tests in each file.
    for file in &files {
        let src = std::fs::read_to_string(file).map_err(|e| {
            DiscoverError::Io(format!("{}: {e}", file.display()))
        })?;

        match discover_tests_in_module(&lang, &src, file) {
            Ok(tests) => {
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

/// A [`ModuleLookup`] wrapper that adds the `assert` module on top of a
/// delegate chain (typically the Kit's stdlib module chain). This lets the
/// test runner evaluate expressions that use `assert.true(false)` etc.
struct TestModuleLookup {
    /// The assert module (assertion primitives).
    assert: AssertModule,
    /// Delegate for all other function lookups (stdlib, fp, meta, etc.).
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl ModuleLookup for TestModuleLookup {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        // Try the assert module first (split qualified name: "assert.true").
        if let Some((module, func)) = name.split_once('.') {
            if module == "assert" {
                if let Some(f) = self.assert.function(func) {
                    return Some(f);
                }
            }
        }
        // Also try the full qualified name against the assert module.
        if let Some(f) = self.assert.function(name) {
            return Some(f);
        }
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
/// assert module on top of the Kit's stdlib module chain.
///
/// This gives test expressions access to both the stdlib (`fn.*`, `math.*`,
/// `str.*`, etc.) and the assertion primitives (`assert.true`,
/// `assert.equal`, etc.).
fn build_test_runtime() -> Result<Runtime, String> {
    let kit = handl_kit_canonical::Kit::new()
        .map_err(|e| format!("failed to assemble canonical kit: {e}"))?;
    let lang = kit.runtime().lang().clone();

    // Get the Kit's module chain (stdlib + fp + meta + hello).
    let kit_modules = kit.runtime().modules().cloned();

    // Build a module lookup that adds assert on top of the Kit's chain.
    let test_modules: Rc<dyn ModuleLookup> = Rc::new(TestModuleLookup {
        assert: AssertModule,
        delegate: kit_modules,
    });

    // Build the eval engine with the test module chain + `self` global.
    let self_val = language_value(&lang);
    let eval_engine = TreeWalkEngine::new()
        .with_modules(test_modules)
        .with_global("self", self_val);

    // Build the runtime with lex, parse, and eval engines.
    let runtime = Runtime::new(lang)
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(eval_engine));

    Ok(runtime)
}

/// Run a test plan.
///
/// Executes each test in order, evaluating the test body with the eval
/// engine. Failures (raised errors) are caught and recorded. No shared
/// state between tests.
pub fn run(plan: &TestPlan) -> RunReport {
    run_with_filter(plan, "")
}

/// Run a test plan with a filter pattern.
///
/// Only tests whose name matches the filter (substring match) are run;
/// others are skipped. An empty filter matches all tests.
pub fn run_with_filter(plan: &TestPlan, filter: &str) -> RunReport {
    let mut report = RunReport::default();

    // Build the test runtime once; reuse for all tests (no shared state
    // between tests — handl has no mutation primitive).
    let runtime = match build_test_runtime() {
        Ok(rt) => rt,
        Err(msg) => {
            // If the runtime can't be built, all tests fail.
            for case in &plan.tests {
                report.outcomes.push(TestOutcome {
                    case: case.clone(),
                    result: TestResult::Fail {
                        message: format!("runtime setup error: {msg}"),
                    },
                });
            }
            return report;
        }
    };

    for case in &plan.tests {
        // Apply filter: skip tests that don't match.
        if !filter.is_empty() && !case.name.contains(filter) {
            report.outcomes.push(TestOutcome {
                case: case.clone(),
                result: TestResult::Skip,
            });
            continue;
        }

        // Evaluate the test body. If it succeeds, the test passes. If it
        // raises an error, the test fails with the error message.
        let result = match runtime.run(&Source::new(&case.body_source)) {
            Ok(_) => TestResult::Pass,
            Err(e) => TestResult::Fail {
                message: e.to_string(),
            },
        };

        report.outcomes.push(TestOutcome {
            case: case.clone(),
            result,
        });
    }

    report
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

/// Format a run report as human-readable output.
///
/// One line per test (`PASS`/`FAIL`/`SKIP`), summary counts, failures with
/// error messages and locations.
pub fn format_report_human(report: &RunReport) -> String {
    let mut out = String::new();

    for outcome in &report.outcomes {
        let module = outcome.case.module.display().to_string();
        match &outcome.result {
            TestResult::Pass => {
                out.push_str(&format!("PASS  {} ({})\n", outcome.case.name, module));
            }
            TestResult::Fail { message } => {
                out.push_str(&format!(
                    "FAIL  {} ({}): {}\n",
                    outcome.case.name, module, message
                ));
            }
            TestResult::Skip => {
                out.push_str(&format!("SKIP  {} ({})\n", outcome.case.name, module));
            }
        }
    }

    // Summary line.
    out.push_str(&format!(
        "\n---\n{} tests: {} passed, {} failed, {} skipped\n",
        report.total(),
        report.passed(),
        report.failed(),
        report.skipped()
    ));

    out
}

/// Format a run report as JSON for CI consumers.
pub fn format_report_json(report: &RunReport) -> String {
    let tests: Vec<serde_json::Value> = report
        .outcomes
        .iter()
        .map(|outcome| {
            let status = match &outcome.result {
                TestResult::Pass => "pass",
                TestResult::Fail { .. } => "fail",
                TestResult::Skip => "skip",
            };
            let mut entry = serde_json::json!({
                "name": outcome.case.name,
                "module": outcome.case.module.display().to_string(),
                "status": status,
            });
            if let TestResult::Fail { message } = &outcome.result {
                entry["message"] = serde_json::Value::String(message.clone());
            }
            entry
        })
        .collect();

    let summary = serde_json::json!({
        "total": report.total(),
        "passed": report.passed(),
        "failed": report.failed(),
        "skipped": report.skipped(),
    });

    serde_json::json!({
        "tests": tests,
        "summary": summary,
    })
    .to_string()
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
                    },
                    result: TestResult::Pass,
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("a.hnl"),
                        name: "test_b".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                    },
                    result: TestResult::Fail {
                        message: "oops".to_string(),
                    },
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("a.hnl"),
                        name: "test_c".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                    },
                    result: TestResult::Skip,
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
    fn format_report_human_basic() {
        let report = RunReport {
            outcomes: vec![
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_a".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                    },
                    result: TestResult::Pass,
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_b".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                    },
                    result: TestResult::Fail {
                        message: "expected true".to_string(),
                    },
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_c".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                    },
                    result: TestResult::Skip,
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
                    },
                    result: TestResult::Pass,
                },
                TestOutcome {
                    case: TestCase {
                        module: PathBuf::from("mod.hnl"),
                        name: "test_b".to_string(),
                        body_source: "1".to_string(),
                        span: Span::default(),
                    },
                    result: TestResult::Fail {
                        message: "oops".to_string(),
                    },
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
}
