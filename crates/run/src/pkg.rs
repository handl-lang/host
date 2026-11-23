//! `PkgScripts` — a package's `pkg.hnl` loaded for script resolution.
//!
//! Discovery walks up from a start path to the nearest `pkg.hnl`
//! ([`handl_platform::config::discover_pkg_dir`]); the manifest's
//! `modules`/`entry` are kept because the cache's *default* input set is
//! "the package's own module sources + `pkg.hnl` itself" (P-003).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use handl_platform::config::{discover_pkg_dir, pkg_entry, pkg_modules, pkg_scripts};

use crate::script::Script;

/// A `pkg.hnl` package manifest loaded for `hnl run`.
#[derive(Debug, Clone)]
pub struct PkgScripts {
    /// The package root — the directory containing `pkg.hnl`. Task
    /// `cwd`s and input/output globs resolve against it.
    pub dir: PathBuf,
    /// The raw `pkg.hnl` text (kept for diagnostics and re-parse).
    pub source: String,
    /// Declared `modules` (for the default cache-input set).
    pub modules: Vec<String>,
    /// Declared `entry` override, when present.
    pub entry: Option<String>,
    /// Scripts by name.
    pub scripts: BTreeMap<String, Script>,
}

impl PkgScripts {
    /// Load `pkg.hnl` from `dir` (a package root, not the file).
    ///
    /// `None` when `dir/pkg.hnl` is absent or unreadable — the same
    /// leniency [`handl_platform::config::resolve_pkg_dependencies`]
    /// applies to a missing manifest. A *malformed* manifest still
    /// yields whatever the lenient `pkg_*` scanners can read (possibly
    /// zero scripts).
    pub fn load(dir: &Path) -> Option<Self> {
        let path = dir.join("pkg.hnl");
        let source = std::fs::read_to_string(&path).ok()?;
        Some(Self::from_source(dir.to_path_buf(), source))
    }

    /// Parse already-read `pkg.hnl` `source` as if it lived in `dir`.
    ///
    /// The test seam: callers that already hold the manifest text (or
    /// want a synthetic package) skip the filesystem read.
    pub fn from_source(dir: PathBuf, source: String) -> Self {
        let modules = pkg_modules(&source);
        let entry = pkg_entry(&source);
        let scripts = pkg_scripts(&source)
            .iter()
            .map(|s| (s.name.clone(), Script::from_pkg(s)))
            .collect();
        PkgScripts {
            dir,
            source,
            modules,
            entry,
            scripts,
        }
    }

    /// Discover the nearest `pkg.hnl` walking up from `start` (a file or
    /// directory — `cwd` for `hnl run <name>`) and load it.
    ///
    /// `None` when no ancestor declares a `pkg.hnl`.
    pub fn discover(start: &Path) -> Option<Self> {
        let dir = discover_pkg_dir(start)?;
        Self::load(&dir)
    }

    /// Look up a script by name.
    pub fn get(&self, name: &str) -> Option<&Script> {
        self.scripts.get(name)
    }

    /// Declared script names, sorted (deterministic listing).
    pub fn names(&self) -> Vec<&str> {
        self.scripts.keys().map(String::as_str).collect()
    }
}
