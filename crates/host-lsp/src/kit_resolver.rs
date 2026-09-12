//! Per-workspace language kit resolution (ADR 0003).
//!
//! The LSP server resolves which language kit (data files) to use for a given
//! file by walking up from the file's path to find a `.handl.toml` manifest.
//! This module implements that resolution with an in-memory cache keyed by the
//! manifest's directory.
//!
//! The `.handl.toml` subset supported here mirrors the extension's `manifest.ts`:
//! section headers (`[language]`), string values (`key = "value"`), comments
//! (`#`), and empty lines. No external TOML dependency is used.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Crate name of the canonical kit, used as the fallback when no manifest is
/// found. `Kit::from_path()` treats this sentinel as "use the compiled-in
/// canonical kit" (loaded at compile time via `include_str!`).
pub const DEFAULT_KIT_NAME: &str = "handl-kit-data-canonical";

/// Name of the manifest file the resolver looks for.
const MANIFEST_FILE: &str = ".handl.toml";

/// The process-global [`KitResolver`] used by [`kit_for_file`].
static GLOBAL_KIT_RESOLVER: OnceLock<KitResolver> = OnceLock::new();

/// Returns the process-global [`KitResolver`], initializing it on first use.
pub fn global_kit_resolver() -> &'static KitResolver {
    GLOBAL_KIT_RESOLVER.get_or_init(KitResolver::new)
}

/// Resolve and build a [`Kit`](handl_kit_canonical::Kit) for `file_path`
/// (ADR 0003 per-file kit loading).
///
/// Walks up from `file_path` to find a `.handl.toml` manifest via the
/// global [`KitResolver`], then constructs a [`Kit`] from the resolved kit
/// path with [`Kit::from_path`](handl_kit_canonical::Kit::from_path). When
/// `file_path` is `None` or empty, or no manifest is found, this falls back
/// to the compiled-in canonical kit.
///
/// # Errors
///
/// Returns [`LangError`](handl_language::LangError) if kit construction fails
/// (which, for the fallback canonical kit, should not happen).
pub fn kit_for_file(
    file_path: Option<&str>,
) -> Result<handl_kit_canonical::Kit, handl_language::LangError> {
    let resolver = global_kit_resolver();
    let resolved = resolver.resolve(file_path.unwrap_or(""));
    handl_kit_canonical::Kit::from_path(&resolved.kit_path)
}

/// A resolved language kit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKit {
    /// Path to the kit's data directory. For the default (compiled-in) kit
    /// this is the crate name sentinel [`DEFAULT_KIT_NAME`]; for a path-based
    /// or crate-name-based kit it is an absolute filesystem path.
    pub kit_path: PathBuf,
    /// Optional version pin read from `.handl.toml`.
    pub version: Option<String>,
    /// The directory the manifest was found in, or `None` for the default kit.
    pub manifest_dir: Option<PathBuf>,
}

impl ResolvedKit {
    /// The default (compiled-in canonical) kit, used when no manifest is found.
    pub fn default_kit() -> Self {
        Self {
            kit_path: PathBuf::from(DEFAULT_KIT_NAME),
            version: None,
            manifest_dir: None,
        }
    }

    /// Returns `true` if this is the default (compiled-in) kit.
    pub fn is_default(&self) -> bool {
        self.manifest_dir.is_none()
    }
}

/// Resolves the language kit for a given file path by walking up the directory
/// tree to find a `.handl.toml` manifest. Results are cached per manifest
/// directory so repeated requests for files in the same workspace reuse the
/// resolved kit.
pub struct KitResolver {
    /// Cache: manifest directory -> resolved kit.
    cache: Mutex<HashMap<PathBuf, ResolvedKit>>,
}

impl Default for KitResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl KitResolver {
    /// Create a new, empty resolver.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve the kit for `file_path`.
    ///
    /// Walks up from the file's parent directory looking for `.handl.toml`.
    /// If found, parses the manifest and resolves the kit (returning a cached
    /// result if this manifest directory has been resolved before). Falls back
    /// to the default kit ([`ResolvedKit::default_kit`]) when no manifest is
    /// found or `file_path` is empty.
    pub fn resolve(&self, file_path: &str) -> ResolvedKit {
        if file_path.is_empty() {
            return ResolvedKit::default_kit();
        }

        let path = Path::new(file_path);
        let start = match path.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => return ResolvedKit::default_kit(),
        };

        // Walk up looking for a manifest.
        let manifest_dir = match find_manifest_dir(start) {
            Some(dir) => dir,
            None => return ResolvedKit::default_kit(),
        };

        // Check the cache first.
        if let Some(cached) = self.cache.lock().unwrap().get(&manifest_dir).cloned() {
            return cached;
        }

        // Parse the manifest and resolve.
        let manifest_path = manifest_dir.join(MANIFEST_FILE);
        let content = match std::fs::read_to_string(&manifest_path) {
            Ok(c) => c,
            Err(_) => return ResolvedKit::default_kit(),
        };
        let parsed = parse_toml(&content);
        let resolved = resolve_manifest(&parsed, &manifest_dir);

        self.cache
            .lock()
            .unwrap()
            .insert(manifest_dir.clone(), resolved.clone());
        resolved
    }

    /// Clear the cache (e.g. when a manifest file changes).
    pub fn invalidate(&self) {
        self.cache.lock().unwrap().clear();
    }

    /// Invalidate a single manifest directory.
    pub fn invalidate_dir(&self, manifest_dir: &Path) {
        self.cache.lock().unwrap().remove(manifest_dir);
    }
}

/// Walk up from `start` one directory at a time, returning the first directory
/// that contains a `.handl.toml` file. Stops at the filesystem root.
fn find_manifest_dir(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf())
    };

    loop {
        let candidate = current.join(MANIFEST_FILE);
        if candidate.is_file() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Resolve a parsed manifest into a [`ResolvedKit`], relative to the manifest's
/// directory. Falls back to the default kit when the `kit` field is missing or
/// cannot be resolved.
fn resolve_manifest(manifest: &TomlManifest, manifest_dir: &Path) -> ResolvedKit {
    let language = match &manifest.language {
        Some(lang) => lang,
        None => return ResolvedKit::default_kit(),
    };

    let kit_name = match &language.kit {
        Some(name) => name,
        None => return ResolvedKit::default_kit(),
    };

    let kit_path = resolve_kit_path(kit_name, manifest_dir);

    ResolvedKit {
        kit_path,
        version: language.version.clone(),
        manifest_dir: Some(manifest_dir.to_path_buf()),
    }
}

/// Resolve the `kit` field of the manifest to a data-directory path.
///
/// - If it starts with `/`, use it as an absolute path directly.
/// - If it starts with `.`, resolve it relative to the manifest directory.
/// - Otherwise treat it as a crate name and look for `crates/<name>/data/`
///   relative to the workspace root (walk up to find a `Cargo.toml` or
///   `crates/` directory). Falls back to the default kit sentinel if not found.
fn resolve_kit_path(kit: &str, manifest_dir: &Path) -> PathBuf {
    if kit.is_empty() {
        return PathBuf::from(DEFAULT_KIT_NAME);
    }

    // Absolute path.
    if kit.starts_with('/') {
        let p = PathBuf::from(kit);
        if p.is_dir() {
            return p;
        }
        return PathBuf::from(DEFAULT_KIT_NAME);
    }

    // Relative path.
    if kit.starts_with('.') {
        let p = manifest_dir.join(kit);
        if p.is_dir() {
            return std::fs::canonicalize(&p).unwrap_or(p);
        }
        return PathBuf::from(DEFAULT_KIT_NAME);
    }

    // Crate name: look for crates/<name>/data/ relative to the workspace root.
    if let Some(workspace_root) = find_workspace_root(manifest_dir) {
        let candidate = workspace_root.join("crates").join(kit).join("data");
        if candidate.is_dir() {
            return std::fs::canonicalize(&candidate).unwrap_or(candidate);
        }
    }

    PathBuf::from(DEFAULT_KIT_NAME)
}

/// Walk up from `start` to find the workspace root — the nearest directory
/// containing a `Cargo.toml` or a `crates/` subdirectory.
fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join("Cargo.toml").is_file() || current.join("crates").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Minimal TOML parser
// ---------------------------------------------------------------------------

/// Parsed `.handl.toml` manifest. Only the `[language]` section is modeled;
/// unknown sections are ignored.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TomlManifest {
    pub language: Option<LanguageSection>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LanguageSection {
    pub kit: Option<String>,
    pub version: Option<String>,
}

/// Parse a `.handl.toml` document into a [`TomlManifest`].
///
/// Supports the same minimal subset as the extension's `manifest.ts`:
/// - Section headers: `[language]` (others ignored)
/// - String values: `key = "value"` (double or single quoted)
/// - Comments: `#` (full-line and trailing, not inside quoted strings)
/// - Empty lines
pub fn parse_toml(content: &str) -> TomlManifest {
    let mut manifest = TomlManifest::default();
    // Track whether we are inside the `[language]` section.
    let mut in_language = false;
    // Accumulate raw key/value pairs for the language section.
    let mut language_raw: Vec<(String, String)> = Vec::new();

    for raw_line in content.lines() {
        let stripped = strip_comment(raw_line);
        let line = stripped.trim();
        if line.is_empty() {
            continue;
        }

        // Section header.
        if let Some(name) = parse_section_header(line) {
            in_language = name == "language";
            continue;
        }

        // key = value (only inside the language section).
        if in_language {
            if let Some((key, value)) = parse_key_value(line) {
                language_raw.push((key, value));
            }
        }
    }

    if !language_raw.is_empty() {
        let mut lang = LanguageSection::default();
        for (key, value) in language_raw {
            match key.as_str() {
                "kit" => lang.kit = Some(value),
                "version" => lang.version = Some(value),
                _ => {}
            }
        }
        manifest.language = Some(lang);
    }

    manifest
}

/// Parse a section header line like `[language]`. Returns the section name if
/// the line is a valid header.
fn parse_section_header(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let inner = trimmed[1..trimmed.len() - 1].trim();
    if inner.is_empty() || inner.contains('[') || inner.contains(']') {
        return None;
    }
    Some(inner.to_string())
}

/// Parse a `key = "value"` line, returning `(key, parsed_string_value)`.
/// Returns `None` if the line has no `=` or the value is not a quoted string.
fn parse_key_value(line: &str) -> Option<(String, String)> {
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    if key.is_empty() {
        return None;
    }
    let value_part = line[eq + 1..].trim();
    let value = parse_string_value(value_part)?;
    Some((key.to_string(), value))
}

/// Parse a quoted string value (`"..."` or `'...'`), handling backslash
/// escapes for double-quoted strings.
fn parse_string_value(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let quote = bytes[0];
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == b'\\' && quote == b'"' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            match next {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                _ => {
                    out.push('\\');
                    out.push(next as char);
                }
            }
            i += 2;
            continue;
        }
        if ch == quote {
            return Some(out);
        }
        out.push(ch as char);
        i += 1;
    }
    // Unterminated string — return what we have.
    Some(out)
}

/// Remove a `#` comment from a line, ignoring `#` inside quoted strings.
fn strip_comment(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut in_string: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if let Some(q) = in_string {
            if ch == b'\\' && q == b'"' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if ch == q {
                in_string = None;
            }
            i += 1;
            continue;
        }
        if ch == b'"' || ch == b'\'' {
            in_string = Some(ch);
            i += 1;
            continue;
        }
        if ch == b'#' {
            return line[..i].to_string();
        }
        i += 1;
    }
    line.to_string()
}
