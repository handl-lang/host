//! Test providers (ADR-0089 §"Providers (extensible from day one)").
//!
//! A [`TestProvider`] turns a source of tests into a [`TestPlan`]. The
//! runner never cares where tests came from; providers are the seam that
//! lets non-filesystem sources (in-memory buffers, IDE documents, naming
//! conventions) plug in later without modifying the runner.
//!
//! Built-in providers:
//!
//! - [`FileSystemProvider`] — discovers `.hnl`/`.handl` files under the
//!   given paths and parses them with the canonical kit ⊕ test-language
//!   fragment.
//! - [`ChangedProvider`] — wraps [`FileSystemProvider`] and only surfaces
//!   tests in files that changed according to `git status --porcelain`.
//!
//! The future `UnitProvider`/`PropertyProvider`/`DoctestProvider`/… split
//! in the ADR is deferred: discovery currently recognizes all declaration
//! kinds in one pass via data-driven `name`/`body` roles (ADR-0081), so
//! splitting by kind is a composition question, not a discovery-mechanism
//! question.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::events::TestEvent;
use crate::runner::{self, DiscoverError, TestCase, TestPlan};

/// A source of tests (ADR-0089 provider API v1).
///
/// Implementations discover tests and return them as a [`TestPlan`];
/// per-module parse failures are recorded in
/// [`TestPlan::parse_errors`]. Providers may emit
/// [`TestEvent::TestDiscovered`] into `events` when a sink is supplied.
pub trait TestProvider {
    /// A short provider name, for diagnostics and `--list` attribution.
    fn name(&self) -> &'static str;

    /// Discover tests under `paths`.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoverError`] if a path does not exist or a file cannot
    /// be read. Per-module parse errors are recorded in
    /// [`TestPlan::parse_errors`] rather than propagated.
    fn discover(
        &self,
        paths: &[PathBuf],
        events: Option<&mut Vec<TestEvent>>,
    ) -> Result<TestPlan, DiscoverError>;
}

/// The default provider: discovers tests in `.hnl`/`.handl` files on the
/// filesystem.
///
/// Covers the ADR's `UnitProvider`/`PropertyProvider`/
/// `IntegrationProvider`/`BenchmarkProvider` inputs in a single pass —
/// every test-kind declares `name`/`body` roles in the test-language
/// fragment, and the `test_*` naming convention remains as a
/// backwards-compat fallback (ADR-0089).
#[derive(Debug, Default, Clone, Copy)]
pub struct FileSystemProvider;

impl TestProvider for FileSystemProvider {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn discover(
        &self,
        paths: &[PathBuf],
        events: Option<&mut Vec<TestEvent>>,
    ) -> Result<TestPlan, DiscoverError> {
        runner::discover_inner(paths, events)
    }
}

/// A provider that only surfaces tests in files changed vs. the git
/// baseline.
///
/// Wraps [`FileSystemProvider`], canonicalizes the input paths, and filters
/// the returned plan to tests whose module file appears in the set of
/// changed `.hnl`/`.handl` files reported by `git status --porcelain`.
/// If git is unavailable or the working directory is not a repository, the
/// provider falls back to discovering all tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChangedProvider;

impl TestProvider for ChangedProvider {
    fn name(&self) -> &'static str {
        "changed"
    }

    fn discover(
        &self,
        paths: &[PathBuf],
        events: Option<&mut Vec<TestEvent>>,
    ) -> Result<TestPlan, DiscoverError> {
        let abs_paths: Vec<PathBuf> = paths
            .iter()
            .map(|p| {
                std::fs::canonicalize(p).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        DiscoverError::NotFound(p.display().to_string())
                    } else {
                        DiscoverError::Io(format!("{}: {e}", p.display()))
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let fs = FileSystemProvider;
        let mut plan = fs.discover(&abs_paths, None)?;

        let changed = match changed_files(&abs_paths) {
            Ok(files) => files,
            Err(e) => {
                eprintln!(
                    "test: warning: failed to determine changed files ({e}); running all discovered tests"
                );
                if let Some(events) = events {
                    for case in &plan.tests {
                        events.push(TestEvent::TestDiscovered {
                            module: case.module.clone(),
                            name: case.name.clone(),
                        });
                    }
                }
                return Ok(plan);
            }
        };

        plan.tests
            .retain(|case: &TestCase| changed.contains(&case.module));
        plan.parse_errors.retain(|(path, _)| changed.contains(path));

        if let Some(events) = events {
            for case in &plan.tests {
                events.push(TestEvent::TestDiscovered {
                    module: case.module.clone(),
                    name: case.name.clone(),
                });
            }
        }

        Ok(plan)
    }
}

/// Determine the set of changed `.hnl`/`.handl` files using git.
///
/// Locates the repository root of each input path independently via
/// `git -C <dir> rev-parse --show-toplevel` (so input paths may span
/// multiple repositories), then runs
/// `git -C <toplevel> status --porcelain --untracked-files=all` per root to
/// list changed, added, renamed, and untracked files. Paths are returned as
/// canonical absolute paths. Falls back to the current directory when
/// `paths` is empty.
fn changed_files(paths: &[PathBuf]) -> Result<HashSet<PathBuf>, String> {
    let mut toplevels: Vec<PathBuf> = Vec::new();
    let mut inputs: Vec<PathBuf> = paths.to_vec();
    if inputs.is_empty() {
        inputs.push(std::env::current_dir().map_err(|e| e.to_string())?);
    }
    for input in &inputs {
        let dir = if input.is_dir() {
            input.clone()
        } else {
            input.parent().unwrap_or(input).to_path_buf()
        };
        let toplevel = repo_root(&dir)?;
        if !toplevels.contains(&toplevel) {
            toplevels.push(toplevel);
        }
    }

    let mut files = HashSet::new();
    for toplevel in &toplevels {
        let output = Command::new("git")
            .arg("-C")
            .arg(toplevel)
            .args(["status", "--porcelain", "--untracked-files=all"])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some(rel) = porcelain_new_path(line) else {
                continue;
            };
            let p = toplevel.join(rel);
            let is_handl = p
                .extension()
                .and_then(|s| s.to_str())
                .map_or(false, |ext| ext == "hnl" || ext == "handl");
            if is_handl {
                let canonical = std::fs::canonicalize(&p).unwrap_or(p);
                files.insert(canonical);
            }
        }
    }
    Ok(files)
}

/// Locate the repository root containing `dir` via
/// `git -C <dir> rev-parse --show-toplevel`.
fn repo_root(dir: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let toplevel = String::from_utf8(output.stdout)
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    // Canonicalize so distinct input paths under the same root deduplicate.
    Ok(std::fs::canonicalize(&toplevel).unwrap_or_else(|_| PathBuf::from(toplevel)))
}

/// Extract the effective path from one `git status --porcelain` line.
///
/// Returns the path relative to the repository root, or `None` for blank
/// lines and entries with an all-whitespace status. Changed/added/
/// untracked/renamed/deleted entries all count: a deleted file simply won't
/// be discovered. For rename/copy entries (`R`/`C`, reported as
/// `old -> new`) the NEW path is returned. C-quoted paths (`"..."`, used by
/// git for names with spaces or special characters) are unescaped.
fn porcelain_new_path(line: &str) -> Option<String> {
    if line.len() < 4 {
        return None;
    }
    let status = &line[..2];
    if status.chars().all(|c| c.is_whitespace()) {
        return None;
    }
    let path_part = line[3..].trim_start();
    if status.contains('R') || status.contains('C') {
        let (_old, new) = split_rename(path_part)?;
        Some(unquote_path(new.trim()))
    } else {
        Some(unquote_path(path_part))
    }
}

/// Split a porcelain rename/copy path pair (`old -> new`) into its two raw
/// (still possibly C-quoted) components. Either side may be quoted; git
/// quotes any path containing spaces or special characters, so an unquoted
/// component never contains the ` -> ` separator.
fn split_rename(path_part: &str) -> Option<(String, String)> {
    if let Some(rest) = path_part.strip_prefix('"') {
        let (old, after) = parse_quoted(rest)?;
        let new = after.trim_start().strip_prefix("->")?.trim_start();
        Some((old, new.to_string()))
    } else {
        let (old, new) = path_part.split_once(" -> ")?;
        Some((old.to_string(), new.to_string()))
    }
}

/// Unquote a git C-style quoted path (`"..."`). Unquoted input is returned
/// unchanged. Inside quotes, standard escapes (`\\`, `\"`, `\n`, `\t`,
/// `\a`, `\b`, `\f`, `\v`, `\r`) and up-to-three-digit octal escapes
/// (used for non-ASCII UTF-8 bytes) are decoded.
fn unquote_path(raw: &str) -> String {
    match raw.strip_prefix('"').and_then(|rest| parse_quoted(rest)) {
        Some((decoded, _)) => decoded,
        None => raw.to_string(),
    }
}

/// Parse the body of a C-quoted string (the text after the opening `"`).
/// Returns the decoded string and the unparsed remainder after the closing
/// quote, or `None` if the quote is unterminated or malformed.
fn parse_quoted(s: &str) -> Option<(String, &str)> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => {
                return Some((String::from_utf8_lossy(&bytes).into_owned(), &s[i + 1..]));
            }
            '\\' => {
                let (_, esc) = chars.next()?;
                match esc {
                    'n' => bytes.push(b'\n'),
                    't' => bytes.push(b'\t'),
                    'r' => bytes.push(b'\r'),
                    'a' => bytes.push(0x07),
                    'b' => bytes.push(0x08),
                    'f' => bytes.push(0x0C),
                    'v' => bytes.push(0x0B),
                    '\\' => bytes.push(b'\\'),
                    '"' => bytes.push(b'"'),
                    '0'..='7' => {
                        // Octal escape: up to two further octal digits.
                        let mut value = esc.to_digit(8).unwrap() as u8;
                        for _ in 0..2 {
                            match chars.peek().copied() {
                                Some((_, d @ '0'..='7')) => {
                                    chars.next();
                                    value = value
                                        .wrapping_mul(8)
                                        .wrapping_add(d.to_digit(8).unwrap() as u8);
                                }
                                _ => break,
                            }
                        }
                        bytes.push(value);
                    }
                    other => {
                        let mut buf = [0u8; 4];
                        bytes.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
                    }
                }
            }
            c => {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    None
}

/// Discover tests via an explicit [`TestProvider`].
///
/// This is the provider-aware generalization of
/// [`crate::runner::discover`], which uses [`FileSystemProvider`] and
/// discards events. Pass `events` to collect
/// [`TestEvent::TestDiscovered`] events.
///
/// # Errors
///
/// Returns [`DiscoverError`] if a path does not exist or a file cannot be
/// read.
pub fn discover_with(
    provider: &dyn TestProvider,
    paths: &[PathBuf],
    events: Option<&mut Vec<TestEvent>>,
) -> Result<TestPlan, DiscoverError> {
    provider.discover(paths, events)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- porcelain path parsing ---------------------------------------------

    #[test]
    fn porcelain_plain_paths() {
        assert_eq!(
            porcelain_new_path(" M src/a.hnl"),
            Some("src/a.hnl".to_string())
        );
        assert_eq!(
            porcelain_new_path("?? new file.handl"),
            Some("new file.handl".to_string())
        );
        assert_eq!(
            porcelain_new_path("A  added.hnl"),
            Some("added.hnl".to_string())
        );
    }

    #[test]
    fn porcelain_skips_uninteresting_lines() {
        assert_eq!(porcelain_new_path(""), None);
        assert_eq!(porcelain_new_path(" M"), None);
        // All-whitespace status columns carry no entry.
        assert_eq!(porcelain_new_path("   a.hnl"), None);
    }

    #[test]
    fn porcelain_rename_uses_new_path() {
        assert_eq!(
            porcelain_new_path("R  old.hnl -> new.hnl"),
            Some("new.hnl".to_string())
        );
        assert_eq!(
            porcelain_new_path("C  src/a.hnl -> src/b.hnl"),
            Some("src/b.hnl".to_string())
        );
    }

    #[test]
    fn porcelain_quoted_paths() {
        // git C-quotes paths containing spaces or special characters.
        assert_eq!(
            porcelain_new_path("?? \"dir/my test.hnl\""),
            Some("dir/my test.hnl".to_string())
        );
        assert_eq!(
            porcelain_new_path(" M \"weird\\tname.hnl\""),
            Some("weird\tname.hnl".to_string())
        );
    }

    #[test]
    fn porcelain_quoted_rename() {
        // Both sides quoted: use the NEW path, unescaped.
        assert_eq!(
            porcelain_new_path("R  \"old name.hnl\" -> \"new name.hnl\""),
            Some("new name.hnl".to_string())
        );
        // Unquoted old side, quoted new side.
        assert_eq!(
            porcelain_new_path("R  old.hnl -> \"new name.hnl\""),
            Some("new name.hnl".to_string())
        );
    }

    #[test]
    fn porcelain_octal_escapes() {
        // Non-ASCII bytes arrive as octal escapes; `\303\251` is UTF-8 `é`.
        assert_eq!(
            unquote_path("\"caf\\303\\251.hnl\""),
            "caf\u{e9}.hnl".to_string()
        );
        assert_eq!(
            porcelain_new_path("?? \"caf\\303\\251.hnl\""),
            Some("caf\u{e9}.hnl".to_string())
        );
    }

    #[test]
    fn unquote_path_passthrough() {
        assert_eq!(unquote_path("plain.hnl"), "plain.hnl");
        assert_eq!(unquote_path("\"a\\\\b.hnl\""), "a\\b.hnl");
        assert_eq!(unquote_path("\"say \\\"hi\\\".hnl\""), "say \"hi\".hnl");
        // Unterminated quote: returned unchanged.
        assert_eq!(unquote_path("\"oops"), "\"oops");
    }
}
