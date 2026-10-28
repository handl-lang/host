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

/// Keys at depth 1 of the last balanced `{ … }` in `src`.
fn record_keys(src: &str) -> Vec<String> {
    let (mut depth, mut start, mut end) = (0usize, None, None);
    for (i, c) in src.char_indices() {
        match c {
            '{' => {
                depth += 1;
                if depth == 1 {
                    start = Some(i);
                }
            }
            '}' => {
                if depth == 1 {
                    end = Some(i);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    let (Some(start), Some(end)) = (start, end) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    let (mut depth, mut current) = (0i32, String::new());
    for c in src[start + 1..end].chars() {
        match c {
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
    let key: String = head
        .trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if !key.is_empty() && !keys.contains(&key) {
        keys.push(key);
    }
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
        let mut modules = HashMap::new();
        for dir in dirs.values() {
            let Ok(defs) = package_modules(dir) else {
                continue;
            };
            for def in defs {
                let funcs: Vec<&str> = def.funcs.iter().map(String::as_str).collect();
                let module = HandlModule::new(def.name.clone(), &def.source, &funcs);
                modules.insert(def.name, Arc::new(module) as Arc<dyn Module + Send + Sync>);
            }
        }
        Self { modules }
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
            std::fs::write(dir.join(rel), src).unwrap();
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
