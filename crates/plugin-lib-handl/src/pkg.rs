//! Package bridge (ADR-0160 P1): a fetched `lib-*` package directory becomes a
//! [`Module`](handl_platform::Module).
//!
//! The platform already owns the fetch/cache half
//! (`handl_platform::fetch_dependencies`, `package_cache_dir`,
//! `resolve_pkg_dependencies`). What was missing was the **dir → module** half:
//! read `pkg.hnl`, map each declared module to its `src/…` file, and build a
//! [`HandlModule`] from it.
//!
//! Two things are deliberately simple here:
//!
//! - **Field discovery is derived, not declared.** A `HandlModule` needs the
//!   record-field names it exposes as callables. Hand-written crates pass a
//!   const (`PRED_FUNCS`); a package has none, so [`discover_funcs`] reads them
//!   back from the source — the `export <name> :=` form (one arrow per file) or
//!   the keys of the record the file evaluates to.
//! **Known limitation.** `discover_funcs` is a scan, not a parser: braces inside
//! string literals can desync the record walk, so a file with `"{"`/`"}"` in a
//! value (or a comment that survives stripping) can yield the wrong key set. The
//! shapes in the unit tests (record-of-arrows, one-arrow-per-file) are safe; the
//! `pred` package needs a parser-grade scan before P2 can rely on it.
//!
//! - **One module per package for now.** A single-module package
//!   (`pkg.hnl` → `src/index.hnl`) is bridged; multi-module packages
//!   (`src/<name>/index.hnl` + intra-package imports, e.g. `str-handl`) need the
//!   import graph wired the way `str_handl_modules` does it — the next P1 slice.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use handl_language::LangError;
use handl_platform::Module;

use crate::{HandlModule, ModuleResolver};

/// One module a package exposes: its name, source text and discovered fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageModuleDef {
    /// The module name (the `pkg.hnl` `modules` entry).
    pub name: String,
    /// The `.hnl` source the module evaluates.
    pub source: String,
    /// Record-field names to expose as callables, in source order.
    pub funcs: Vec<String>,
}

/// Read a package directory and describe the modules it exposes.
///
/// # Errors
///
/// Returns [`LangError::engine`] when `pkg.hnl` is missing or unreadable, when it
/// declares no modules, or when a declared module's file cannot be read.
pub fn package_modules(pkg_dir: &Path) -> Result<Vec<PackageModuleDef>, LangError> {
    let manifest_path = pkg_dir.join("pkg.hnl");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|e| {
        LangError::engine(format!("package_modules: {}: {e}", manifest_path.display()))
    })?;
    let declared = handl_platform::pkg_modules(&manifest);
    if declared.is_empty() {
        return Err(LangError::engine(format!(
            "package_modules: {} declares no modules",
            manifest_path.display()
        )));
    }
    let entry = handl_platform::pkg_entry(&manifest);
    let mut out = Vec::with_capacity(declared.len());
    for name in &declared {
        let Some(rel) = handl_platform::pkg_module_path(&declared, entry.as_deref(), name) else {
            continue;
        };
        let path = pkg_dir.join(&rel);
        let source = std::fs::read_to_string(&path)
            .map_err(|e| LangError::engine(format!("package_modules: {}: {e}", path.display())))?;
        out.push(PackageModuleDef {
            name: name.clone(),
            funcs: discover_funcs(&source),
            source,
        });
    }
    Ok(out)
}

/// The callable names a `.hnl` source exposes, in source order.
///
/// Two shapes are recognised: per-file arrows (`export <name> := …`, the
/// `lib-<port>-handl` convention) and a record (`{ f: …, g: … }`, every key at
/// the record's top level). Comments (`-- …`) are ignored; a source matching
/// neither shape yields an empty list rather than an error.
pub fn discover_funcs(src: &str) -> Vec<String> {
    let mut exports = Vec::new();
    for line in src.lines() {
        let line = match line.find("--") {
            Some(i) => &line[..i],
            None => line,
        };
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("export ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && !exports.contains(&name) {
                exports.push(name);
            }
        }
    }
    if !exports.is_empty() {
        return exports;
    }
    record_keys(src)
}

/// The callable record a `.hnl` source exports: keys of the **last** brace block
/// that declares at least two `name : value` fields.
///
/// Why not simply "the last block": a package file declares internal helpers
/// before the exported record (`rev :: …`, `prefix_match :: …`), and those
/// helpers live at the *outer* block's top level while the exported record is a
/// nested literal at the end. Two filters together pick it out:
///
/// - only entries with a **single** `:` count (`name :: …` is a definition, not
///   a record field), and
/// - blocks are considered from the end of the file backwards, taking the first
///   with at least two such fields.
fn record_keys(src: &str) -> Vec<String> {
    let spans = brace_spans(src);
    let mut single = None;
    for (start, end) in spans.iter().rev() {
        let keys = field_keys(&src[start + 1..*end]);
        if keys.len() >= 2 {
            return keys;
        }
        if keys.len() == 1 && single.is_none() {
            single = Some(keys);
        }
    }
    single.unwrap_or_default()
}

/// Byte spans of every balanced `{ … }` block in `src`, in source order.
///
/// String literals (`"…"`, with `\` escapes) and line comments (`-- …`) are
/// skipped so a brace inside either cannot desync the stack.
fn brace_spans(src: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut in_str = false;
    let mut escaped = false;
    let mut in_comment = false;
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            i += 1;
            continue;
        }
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            '-' if bytes.get(i + 1) == Some(&b'-') => {
                in_comment = true;
                i += 2;
                continue;
            }
            '"' => in_str = true,
            '{' => stack.push(i),
            '}' => {
                if let Some(open) = stack.pop() {
                    spans.push((open, i));
                }
            }
            _ => {}
        }
        i += 1;
    }
    spans
}

/// `name` keys of the fields a block body declares with a single `:`.
fn field_keys(body: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut in_str = false;
    let mut escaped = false;
    let mut in_comment = false;
    let chars: Vec<char> = body.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let c = chars[idx];
        idx += 1;
        if in_comment {
            if c == '\n' {
                in_comment = false;
            }
            continue;
        }
        if !in_str && c == '-' && chars.get(idx) == Some(&'-') {
            in_comment = true;
            idx += 1;
            continue;
        }
        if in_str {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                current.push(c);
            }
            '{' | '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            '}' | ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                push_key(&mut keys, &current);
                current.clear();
            }
            _ => current.push(c),
        }
    }
    push_key(&mut keys, &current);
    keys
}

/// Take the identifier before the first `:` of one record entry.
fn push_key(keys: &mut Vec<String>, entry: &str) {
    let Some((head, _)) = entry.split_once(':') else {
        return;
    };
    // `name :: …` is a definition (an internal helper), not a record field.
    if head.trim_end().ends_with(':') {
        return;
    }
    let key: String = head
        .trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if !key.is_empty() && !keys.contains(&key) {
        keys.push(key);
    }
}

/// The module name an import path names: its first path segment
/// (`"libc/socket"` -> `libc`).
fn normalize_module_path(spec: &str) -> &str {
    spec.split('/').next().unwrap_or(spec)
}

/// Build one module of a package as a [`HandlModule`].
///
/// The one-call form a consumer uses after dropping its `include_str!` shell:
/// the source and its field names both come from the package.
///
/// # Errors
///
/// Returns [`LangError::engine`] when the directory is not a package or does not
/// declare `name`.
pub fn module_from_dir(pkg_dir: &Path, name: &str) -> Result<HandlModule, LangError> {
    let defs = package_modules(pkg_dir)?;
    let def = defs.iter().find(|d| d.name == name).ok_or_else(|| {
        LangError::engine(format!(
            "module_from_dir: {} declares no module `{name}`",
            pkg_dir.display()
        ))
    })?;
    let funcs: Vec<&str> = def.funcs.iter().map(String::as_str).collect();
    Ok(HandlModule::new(def.name.clone(), &def.source, &funcs))
}

/// A [`ModuleResolver`] over a set of fetched package directories.
///
/// Build it from the name→dir map the platform already produces
/// (`handl_platform::resolve_pkg_dependencies`); modules are constructed once,
/// so resolution is a hash lookup.
#[derive(Default)]
pub struct PackageResolver {
    modules: HashMap<String, Arc<dyn Module + Send + Sync>>,
}

impl PackageResolver {
    /// Build a resolver over `{ module name → package dir }`.
    ///
    /// A directory that is not a package (no `pkg.hnl`) is skipped rather than
    /// failing the set: the platform's map includes cache slots a failed or
    /// disabled fetch never populated.
    pub fn from_dirs(dirs: &HashMap<String, PathBuf>) -> Self {
        let mut modules: HashMap<String, Arc<dyn Module + Send + Sync>> = HashMap::new();
        for dir in dirs.values() {
            let Ok(defs) = package_modules(dir) else {
                continue;
            };
            for def in order_by_imports(&defs) {
                let funcs: Vec<&str> = def.funcs.iter().map(String::as_str).collect();
                let mut module = HandlModule::new(def.name.clone(), &def.source, &funcs);
                for spec in crate::import_specs(&def.source) {
                    let dep_name = normalize_module_path(&spec);
                    if let Some(dep) = modules.get(dep_name) {
                        module = module.with_dependency_arc(Arc::clone(dep));
                    }
                }
                modules.insert(def.name, Arc::new(module) as Arc<dyn Module + Send + Sync>);
            }
        }
        Self { modules }
    }

    /// Build a resolver for the package in `dir`: fetch its declared deps (the
    /// platform does the `git clone` + cache), resolve the name→dir map, then
    /// load every package that map names.
    ///
    /// Fetching is best-effort: `HANDL_PKG_FETCH=off` (or no network) leaves the
    /// cache slots empty, `from_dirs` skips them, and the caller sees the same
    /// "unresolved module" error it would have seen before the hook existed.
    pub fn from_manifest_dir(dir: &Path) -> Self {
        let _ = handl_platform::fetch_dependencies(dir);
        match handl_platform::resolve_pkg_dependencies(dir) {
            Ok(dirs) => Self::from_dirs(&dirs),
            Err(_) => Self::default(),
        }
    }

    /// The number of modules this resolver answers for.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Whether no package could be loaded.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

impl ModuleResolver for PackageResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        self.modules.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, modules: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hnl-pkg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("pkg.hnl"), format!("{{ modules: [{modules}] }}\n")).unwrap();
        for (rel, src) in files {
            let path = dir.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, src).unwrap();
        }
        dir
    }

    #[test]
    fn discovers_record_keys() {
        let src = "-- pkg\n{\n  always: c ~> true,\n  field: (n, m) ~> m,\n  and: (p, q) ~> p,\n}\n";
        assert_eq!(discover_funcs(src), vec!["always", "field", "and"]);
    }

    #[test]
    fn discovers_exported_arrows() {
        let src = "--- doc\nexport contains := (s, pat) ~> do { find(s, pat) >= 0 }\n";
        assert_eq!(discover_funcs(src), vec!["contains"]);
    }

    #[test]
    fn ignores_nesting_in_record_values() {
        let src = "{ a: do { x }, b: { inner: 1 } }\n";
        assert_eq!(discover_funcs(src), vec!["a", "b"]);
    }

    #[test]
    fn empty_when_neither_shape() {
        assert!(discover_funcs("--- just a doc\n42\n").is_empty());
    }

    #[test]
    fn package_modules_reads_manifest_and_source() {
        let dir = pkg("one", "\"pred\"", &[("src/index.hnl", "{ always: c ~> true }\n")]);
        let defs = package_modules(&dir).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "pred");
        assert_eq!(defs[0].funcs, vec!["always"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let dir = std::env::temp_dir().join(format!("hnl-pkg-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(package_modules(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn orders_modules_by_intra_package_imports() {
        // `b` imports `a`, declared in reverse order: the resolver must emit `a`
        // first so `b` can wire it as a dependency.
        let dir = pkg(
            "multi",
            "\"b\", \"a\"",
            &[
                ("src/b/index.hnl", "from \"a\" import *
op \"b.go\" : Unit -> Unit
"),
                ("src/a/index.hnl", "{ a: c ~> true }
"),
            ],
        );
        let defs = package_modules(&dir).unwrap();
        let ordered = order_by_imports(&defs);
        assert_eq!(
            ordered.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ignores_braces_inside_string_literals() {
        // The shape that broke the first scan: a matcher arm carrying a brace literal.
        let src = r#"{
  glob: (p) ~> if p == "{" { true } else { false },
  exact: (v) ~> v == v,
}
"#;
        assert_eq!(discover_funcs(src), vec!["glob", "exact"]);
    }

    /// Opt-in (`cargo test -- --ignored`): bridges the real sibling checkout.
    #[test]
    #[ignore = "needs the ../../../lib-pred sibling checkout"]
    fn bridges_the_real_lib_pred_package() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../lib-pred");
        let defs = package_modules(&dir).expect("lib-pred is a package");
        assert_eq!(defs[0].name, "pred");
        for f in ["always", "never", "and", "field", "exact", "glob", "build", "matches"] {
            assert!(defs[0].funcs.iter().any(|x| x == f), "missing {f}: {:?}", defs[0].funcs);
        }
    }

    #[test]
    fn resolver_skips_non_packages() {
        let good = pkg("good", "\"pred\"", &[("src/index.hnl", "{ always: c ~> true }\n")]);
        let bad = std::env::temp_dir().join(format!("hnl-pkg-bad-{}", std::process::id()));
        std::fs::create_dir_all(&bad).unwrap();
        let mut dirs = HashMap::new();
        dirs.insert("pred".to_string(), good.clone());
        dirs.insert("absent".to_string(), bad.clone());
        let resolver = PackageResolver::from_dirs(&dirs);
        assert_eq!(resolver.len(), 1);
        assert!(resolver.resolve("pred").is_some());
        assert!(resolver.resolve("absent").is_none());
        let _ = std::fs::remove_dir_all(&good);
        let _ = std::fs::remove_dir_all(&bad);
    }
}

/// Order `defs` so a module comes after every sibling it imports.
///
/// A cycle — or an import of something outside the package — cannot be ordered;
/// such modules stay in declaration order and their unresolved imports are left
/// unwired (an import error at use time, never a build-time panic).
fn order_by_imports(defs: &[PackageModuleDef]) -> Vec<PackageModuleDef> {
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    let mut emitted: Vec<usize> = Vec::with_capacity(defs.len());
    while emitted.len() < defs.len() {
        let before = emitted.len();
        for (i, def) in defs.iter().enumerate() {
            if emitted.contains(&i) {
                continue;
            }
            let ready = crate::import_specs(&def.source).iter().all(|spec| {
                let dep = normalize_module_path(spec);
                !names.contains(&dep) || defs.iter().position(|d| d.name == dep).is_some_and(|j| emitted.contains(&j))
            });
            if ready {
                emitted.push(i);
            }
        }
        if emitted.len() == before {
            // Cycle or mutual import: keep the rest in declaration order.
            for i in 0..defs.len() {
                if !emitted.contains(&i) {
                    emitted.push(i);
                }
            }
        }
    }
    emitted.into_iter().map(|i| defs[i].clone()).collect()
}
