//! File loader: reads `.hnl` arrow files, parses them into `Term` IR,
//! resolves cross-file references, and bridges to the inference engine.
//!
//! This closes the gap between source files and the inference engine
//! (ADR-0069): a user writes a `.hnl` file, and `infer_file` provides the
//! path from that file to `infer_refinement(scheme, registry)`.
//!
//! # Cross-file reference resolution
//!
//! When a `.hnl` file references a named arrow (e.g. `sort.hnl` calls
//! `insert(...)`), the parser produces an `App { callee: arr("insert") }`
//! node. The loader resolves these by loading the referenced arrow's file
//! and inlining its `Term` graph -- matching the hand-built graphs in
//! `e2e_tests.rs` where `sort` inlines `insert`'s full graph.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use handl_language::{ArrSource, Term, Value};

use plugin_engine::TermData;

use crate::arrow_parser::{
    ExportDecl, ImportDecl, ParsedFile, parse_arrow_module, parse_arrow_module_no_rewrite,
};
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::{Contract, Pred, RefinementResult};

// ============================================================================
// Public API
// ============================================================================

/// Load a single `.hnl` arrow file and return its `Term` graph.
///
/// For single-expression files, returns the arrow directly.
/// For multi-name files, returns the first named arrow (or the whole record
/// if there's only one).
pub fn load_arrow_file(path: &str) -> Result<Term, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {}", path, e))?;
    let parsed = parse_arrow_module_no_rewrite(&content)?;

    match parsed.arrows {
        ParsedFile::Single(scheme) => Ok(scheme),
        ParsedFile::Named(map) => {
            // Return the first named arrow
            map.into_iter()
                .next()
                .map(|(_, scheme)| scheme)
                .ok_or_else(|| "multi-name file has no arrows".to_string())
        }
    }
}

/// Load a `.hnl` file and return all named arrows as a `HashMap`.
///
/// For single-expression files, returns a map with a single entry keyed by
/// the file's stem name.
/// For multi-name files, returns all named arrows.
pub fn load_arrow_file_named(path: &str) -> Result<HashMap<String, Term>, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {}", path, e))?;
    let parsed = parse_arrow_module_no_rewrite(&content)?;

    match parsed.arrows {
        ParsedFile::Single(scheme) => {
            // Use the file stem as the key
            let name = std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("arrow")
                .to_string();
            let mut map = HashMap::new();
            map.insert(name, scheme);
            Ok(map)
        }
        ParsedFile::Named(map) => Ok(map),
    }
}

/// Load and infer: parse `.hnl` -> `Term` -> resolve references -> `infer_refinement`.
///
/// This is the end-to-end path from a `.hnl` source file to an inferred
/// contract. Cross-file references (e.g. `sort` referencing `insert`) are
/// resolved by loading all arrow files from the same arrows directory.
pub fn infer_file(path: &str, registry: &ContractRegistry) -> Result<RefinementResult, String> {
    let scheme = load_arrow_file(path)?;

    // Build a graph map from the arrows root for reference resolution
    let graph_map = if let Some(root) = find_arrows_root(path) {
        load_all_arrows(&root).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Resolve App nodes by inlining referenced graphs
    let resolved = resolve_apps(&scheme, &graph_map);

    // Infer
    infer_refinement(&resolved, registry)
}

/// Load all `.hnl` arrow files from a directory (recursively) and return a
/// name -> `Term` graph map.
///
/// The key is the file stem (e.g. `insert.hnl` -> `"insert"`).
pub fn load_all_arrows(dir: &str) -> Result<HashMap<String, Term>, String> {
    let mut map = HashMap::new();
    walk_dir(dir, &mut map)?;
    Ok(map)
}

/// Resolve `App` nodes in a `Term` by inlining referenced graphs.
///
/// When an `App { callee: arr("name") }` is found and `"name"` exists in the
/// graph map, the App is replaced with the referenced graph (recursively
/// resolved). This matches the hand-built graphs in `e2e_tests.rs` where
/// `sort` inlines `insert`'s full graph.
pub fn resolve_apps(scheme: &Term, graph_map: &HashMap<String, Term>) -> Term {
    resolve_apps_inner(scheme, graph_map, 0)
}

/// Find the arrows root directory from a file path.
///
/// Looks for `/handl-packages/` in the path and returns everything up to and
/// including it. Returns `None` if the path doesn.t contain `/handl-packages/`.
fn find_arrows_root(path: &str) -> Option<String> {
    let marker = "/handl-packages/";
    let idx = path.find(marker)?;
    Some(path[..idx + marker.len()].to_string())
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Recursively walk a directory and load all `.hnl` files into the map.
///
/// Both single-expression files and multi-name files are loaded. Files that
/// fail to parse are silently skipped (the parser is minimal and may not
/// handle all syntax).
fn walk_dir(dir: &str, map: &mut HashMap<String, Term>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("failed to read directory {}: {}", dir, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            if path.file_name().map(|n| n == "contracts").unwrap_or(false) {
                continue;
            }
            walk_dir(path.to_str().unwrap_or(""), map)?;
        } else if path.extension().map(|e| e == "hnl").unwrap_or(false) {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue, // skip unreadable files
            };
            // Parse — use module-aware parser (strips import/export/op/handl/cap
            // declarations) but WITHOUT import rewriting, so bare name references
            // are preserved for `resolve_apps`. Skip files that fail.
            if let Ok(parsed) = parse_arrow_module_no_rewrite(&content) {
                match parsed.arrows {
                    ParsedFile::Single(scheme) => {
                        map.insert(name, scheme);
                    }
                    ParsedFile::Named(named_map) => {
                        for (k, v) in named_map {
                            map.insert(k, v);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Inner resolution with a depth limit to prevent infinite recursion.
fn resolve_apps_inner(scheme: &Term, graph_map: &HashMap<String, Term>, depth: usize) -> Term {
    // Depth limit to prevent infinite recursion on circular references
    if depth > 16 {
        return scheme.clone();
    }

    let td = TermData::from_term(scheme);
    resolve_apps_inner_td(&td, graph_map, depth)
        .to_term()
        .expect("resolve_apps_inner: TermData -> Term conversion failed")
}

/// Data-driven counterpart of [`resolve_apps_inner`], operating on
/// [`TermData`] and returning [`TermData`].
fn resolve_apps_inner_td(
    td: &TermData,
    graph_map: &HashMap<String, Term>,
    depth: usize,
) -> TermData {
    // Depth limit to prevent infinite recursion on circular references
    if depth > 16 {
        return td.clone();
    }

    match td.tag() {
        "Op" => td.clone(),

        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            TermData::seq(
                resolve_apps_inner_td(&fst, graph_map, depth),
                resolve_apps_inner_td(&snd, graph_map, depth),
            )
        }

        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            TermData::first(resolve_apps_inner_td(&inner, graph_map, depth))
        }

        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            TermData::choice(
                resolve_apps_inner_td(&l, graph_map, depth),
                resolve_apps_inner_td(&r, graph_map, depth),
            )
        }

        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            TermData::loop_feedback(resolve_apps_inner_td(&body, graph_map, depth))
        }

        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            // Check if the callee is a reference to a named arrow
            if callee.is_tag("Op") {
                if let Some(op) = callee.get_str("name") {
                    if let Some(graph) = graph_map.get(op) {
                        // Inline the referenced graph (recursively resolved)
                        let graph_td = TermData::from_term(graph);
                        return resolve_apps_inner_td(&graph_td, graph_map, depth + 1);
                    }
                }
            }
            // If not found, keep the App as-is (inference returns True for App)
            let args: Vec<TermData> = td
                .get_term_list("args")
                .iter()
                .map(|a| resolve_apps_inner_td(a, graph_map, depth))
                .collect();
            TermData::app(resolve_apps_inner_td(&callee, graph_map, depth), args)
        }

        // `handle body with target = impl_` (ADR-0076): eliminate the handler
        // at compile time, then resolve apps in the resulting handler-free
        // graph.
        "Handle" => {
            let eliminated = td
                .to_term()
                .expect("Handle: TermData -> Term conversion failed")
                .eliminate_handlers();
            let eliminated_td = TermData::from_term(&eliminated);
            resolve_apps_inner_td(&eliminated_td, graph_map, depth)
        }

        "Arr" | "Child" => td.clone(),
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            TermData::iter(
                resolve_apps_inner_td(&fuel, graph_map, depth),
                resolve_apps_inner_td(&predicate, graph_map, depth),
                resolve_apps_inner_td(&step, graph_map, depth),
                resolve_apps_inner_td(&initial, graph_map, depth),
            )
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            // Reconstruct via to_term to preserve the witness field exactly.
            let witness_term = td.to_term().expect("Rewrite: TermData -> Term failed");
            if let Term::Rewrite { witness, .. } = witness_term {
                TermData::rewrite(
                    resolve_apps_inner_td(&source, graph_map, depth),
                    resolve_apps_inner_td(&target, graph_map, depth),
                    &witness,
                )
            } else {
                unreachable!()
            }
        }
        _ => td.clone(),
    }
}

// ============================================================================
// Module system: resolvers, loader, and normalization strategies (ADR-0069)
// ============================================================================

// --- Section 1: ResolvedModule and errors ---

/// A resolved module: its logical path, source text, and origin label
/// (e.g. "file", "memory", "chain", "cache"). Produced by a [`ModuleResolver`].
///
/// (ADR-0069 §4 — the loader consumes resolved modules to populate caches.)
#[derive(Debug, Clone)]
pub struct ResolvedModule {
    /// The logical module path that was resolved.
    pub path: String,
    /// The source text of the module.
    pub source: String,
    /// Where the module was found (e.g. "file", "memory").
    pub origin: String,
}

/// Error kind for module resolution failures.
#[derive(Debug, Clone)]
pub enum ResolveErrorKind {
    /// The path was not found by any resolver in the chain.
    NotFound,
    /// An I/O error occurred while reading the module source.
    IoError(String),
    /// The host denied the capability required to resolve this path.
    Disallowed,
}

/// A resolution error: the path that failed plus a kind.
#[derive(Debug, Clone)]
pub struct ResolveError {
    /// The path that failed to resolve.
    pub path: String,
    /// The kind of resolution failure.
    pub kind: ResolveErrorKind,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.kind {
            ResolveErrorKind::NotFound => write!(f, "module not found: {}", self.path),
            ResolveErrorKind::IoError(msg) => {
                write!(f, "io error resolving {}: {}", self.path, msg)
            }
            ResolveErrorKind::Disallowed => {
                write!(f, "capability denied: cannot resolve {}", self.path)
            }
        }
    }
}

// --- Section 2: ModuleResolver trait + built-in resolvers ---

/// The module resolver trait. Resolution is a host capability boundary
/// (ADR-0069 §4): the resolver set determines which paths are importable.
/// `resolve(&self)` is immutable; caches use interior mutability (`RefCell`).
pub trait ModuleResolver {
    /// Resolve a module path to its source text.
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError>;
    /// A short label for where this resolver finds modules ("file", "memory",
    /// "chain", "cache").
    fn origin(&self) -> &'static str;
}

/// `FileResolver` — resolves `path` to `<root>/<path>.hnl` (appending `.hnl`
/// if the path has no extension). Reads the file from disk. This is the
/// default resolver for compiled builds (ADR-0069 §7).
pub struct FileResolver {
    /// The root directory to resolve module paths from.
    pub root: PathBuf,
}

impl FileResolver {
    /// Create a new `FileResolver` rooted at `root`.
    pub fn new(root: PathBuf) -> Self {
        FileResolver { root }
    }
}

impl ModuleResolver for FileResolver {
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError> {
        let mut file_path = self.root.clone();
        file_path.push(path);
        if file_path.extension().is_none() {
            file_path.set_extension("hnl");
        }
        match std::fs::read_to_string(&file_path) {
            Ok(source) => Ok(ResolvedModule {
                path: path.to_string(),
                source,
                origin: "file".to_string(),
            }),
            Err(e) => {
                let kind = if e.kind() == std::io::ErrorKind::NotFound {
                    ResolveErrorKind::NotFound
                } else {
                    ResolveErrorKind::IoError(e.to_string())
                };
                Err(ResolveError {
                    path: path.to_string(),
                    kind,
                })
            }
        }
    }

    fn origin(&self) -> &'static str {
        "file"
    }
}

/// `MemoryResolver` — resolves paths against an in-memory map of path to
/// source. Used for REPL, tests, and virtual environments.
pub struct MemoryResolver {
    /// The in-memory map of module path to source text.
    pub modules: HashMap<String, String>,
}

impl MemoryResolver {
    /// Create a new empty `MemoryResolver`.
    pub fn new() -> Self {
        MemoryResolver {
            modules: HashMap::new(),
        }
    }

    /// Insert a module into the in-memory map.
    pub fn insert(&mut self, path: &str, source: &str) {
        self.modules.insert(path.to_string(), source.to_string());
    }
}

impl Default for MemoryResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleResolver for MemoryResolver {
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError> {
        match self.modules.get(path) {
            Some(source) => Ok(ResolvedModule {
                path: path.to_string(),
                source: source.clone(),
                origin: "memory".to_string(),
            }),
            None => Err(ResolveError {
                path: path.to_string(),
                kind: ResolveErrorKind::NotFound,
            }),
        }
    }

    fn origin(&self) -> &'static str {
        "memory"
    }
}

/// `ChainResolver` — tries each resolver in order, returning the first `Ok`.
/// If all fail, returns the last error (or `NotFound` if empty).
pub struct ChainResolver {
    /// The ordered list of resolvers to try.
    pub resolvers: Vec<Box<dyn ModuleResolver>>,
}

impl ChainResolver {
    /// Create a new `ChainResolver` from an ordered list of resolvers.
    pub fn new(resolvers: Vec<Box<dyn ModuleResolver>>) -> Self {
        ChainResolver { resolvers }
    }
}

impl ModuleResolver for ChainResolver {
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError> {
        let mut last_err = ResolveError {
            path: path.to_string(),
            kind: ResolveErrorKind::NotFound,
        };
        for r in &self.resolvers {
            match r.resolve(path) {
                Ok(m) => return Ok(m),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    fn origin(&self) -> &'static str {
        "chain"
    }
}

/// `CachingResolver` — wraps another resolver and caches resolved modules by
/// path. Uses `RefCell` since `resolve(&self)` is immutable.
pub struct CachingResolver {
    /// The inner resolver to delegate to on cache miss.
    pub inner: Box<dyn ModuleResolver>,
    cache: RefCell<HashMap<String, ResolvedModule>>,
}

impl CachingResolver {
    /// Create a new `CachingResolver` wrapping `inner`.
    pub fn new(inner: Box<dyn ModuleResolver>) -> Self {
        CachingResolver {
            inner,
            cache: RefCell::new(HashMap::new()),
        }
    }
}

impl ModuleResolver for CachingResolver {
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError> {
        if let Some(m) = self.cache.borrow().get(path) {
            return Ok(m.clone());
        }
        match self.inner.resolve(path) {
            Ok(m) => {
                self.cache.borrow_mut().insert(path.to_string(), m.clone());
                Ok(m)
            }
            Err(e) => Err(e),
        }
    }

    fn origin(&self) -> &'static str {
        self.inner.origin()
    }
}

// --- Section 3: LoadError ---

/// Errors that can occur during module loading or normalization.
#[derive(Debug, Clone)]
pub enum LoadError {
    /// A resolution failure (path not found, io error, capability denied).
    Resolve(ResolveError),
    /// A parse error (the source resolved but did not parse).
    Parse(String),
    /// An import cycle was detected. The vector lists the cycle path.
    Cycle(Vec<String>),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LoadError::Resolve(e) => write!(f, "resolve error: {}", e),
            LoadError::Parse(msg) => write!(f, "parse error: {}", msg),
            LoadError::Cycle(path) => {
                write!(f, "import cycle detected: {}", path.join(" -> "))
            }
        }
    }
}

// --- Section 4: LoadedModule and ModuleLoader (ADR-0069 §4, Strategy C) ---

/// The result of loading a module: its named arrow graphs and inferred
/// contracts, cloned from the loader's caches.
#[derive(Debug, Clone)]
pub struct LoadedModule {
    /// The named arrows in this module (path -> Term).
    pub arrows: HashMap<String, Term>,
    /// The inferred contracts for each arrow (one per arrow, in arbitrary
    /// order).
    pub contracts: Vec<Contract>,
}

/// `ModuleLoader` maintains two caches keyed by module path (ADR-0069 §4):
/// - **Graph cache**: `path → named arrows` (the parsed arrow graphs).
/// - **Contract cache**: `path → contracts` (the inferred contracts per arrow).
///
/// On first load, the loader resolves the source, parses it (with
/// import/export extraction), infers contracts, and stores both in the
/// caches. Subsequent loads return cached clones. This is Strategy C
/// (runtime module loading, lazy + cached) — see ADR-0069 §5.
pub struct ModuleLoader {
    resolver: Box<dyn ModuleResolver>,
    /// Graph cache: module path → named arrows.
    pub graph_cache: RefCell<HashMap<String, HashMap<String, Term>>>,
    /// Contract cache: module path → contracts per arrow.
    pub contract_cache: RefCell<HashMap<String, Vec<Contract>>>,
    /// Private cache for module metadata (imports/exports).
    meta_cache: RefCell<HashMap<String, (Vec<ImportDecl>, Vec<ExportDecl>)>>,
}

impl ModuleLoader {
    /// Create a new `ModuleLoader` with the given resolver.
    pub fn new(resolver: Box<dyn ModuleResolver>) -> Self {
        ModuleLoader {
            resolver,
            graph_cache: RefCell::new(HashMap::new()),
            contract_cache: RefCell::new(HashMap::new()),
            meta_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Load a module by path. On first access: resolve → parse → infer
    /// contracts → cache both. On subsequent access: return cached clones.
    /// Returns [`LoadedModule`] (arrows + contracts).
    pub fn load(&self, path: &str) -> Result<LoadedModule, LoadError> {
        // Cache hit?
        if let Some(arrows) = self.graph_cache.borrow().get(path) {
            let contracts = self
                .contract_cache
                .borrow()
                .get(path)
                .cloned()
                .unwrap_or_default();
            return Ok(LoadedModule {
                arrows: arrows.clone(),
                contracts,
            });
        }

        // Resolve the source.
        let resolved = self.resolver.resolve(path).map_err(LoadError::Resolve)?;

        // Parse with module-aware parser (extracts imports/exports).
        let parsed = parse_arrow_module(&resolved.source).map_err(LoadError::Parse)?;

        // Convert ParsedFile to HashMap<String, Term>.
        let arrows = match parsed.arrows {
            ParsedFile::Single(scheme) => {
                let mut map = HashMap::new();
                map.insert("main".to_string(), scheme);
                map
            }
            ParsedFile::Named(map) => map,
        };

        // Infer contracts for each arrow using the standard registry.
        let registry = ContractRegistry::standard();
        let mut contracts: Vec<Contract> = Vec::new();
        for (_name, scheme) in &arrows {
            if let Ok(result) = infer_refinement(scheme, &registry) {
                contracts.push(refinement_to_contract(&result));
            }
        }

        // Cache everything.
        self.graph_cache
            .borrow_mut()
            .insert(path.to_string(), arrows.clone());
        self.contract_cache
            .borrow_mut()
            .insert(path.to_string(), contracts.clone());
        self.meta_cache.borrow_mut().insert(
            path.to_string(),
            (parsed.imports.clone(), parsed.exports.clone()),
        );

        Ok(LoadedModule { arrows, contracts })
    }

    /// Load a specific named arrow from a module. If the module has a single
    /// arrow, it is returned regardless of `name`.
    pub fn load_arrow(&self, path: &str, name: &str) -> Result<Term, LoadError> {
        let loaded = self.load(path)?;
        if let Some(scheme) = loaded.arrows.get(name) {
            return Ok(scheme.clone());
        }
        // If single-arrow module, return that arrow.
        if loaded.arrows.len() == 1 {
            return Ok(loaded.arrows.values().next().unwrap().clone());
        }
        Err(LoadError::Parse(format!(
            "arrow '{}' not found in module '{}'",
            name, path
        )))
    }

    /// Return the public names of a module. If the module has export
    /// declarations, only exported names are returned (with re-exports
    /// expanded). If there are no exports, all named arrows are public.
    pub fn public_names(&self, path: &str) -> Result<Vec<String>, LoadError> {
        let loaded = self.load(path)?;
        let (imports, exports) = self
            .meta_cache
            .borrow()
            .get(path)
            .cloned()
            .unwrap_or_default();

        if exports.is_empty() {
            // No exports = all names public.
            return Ok(loaded.arrows.keys().cloned().collect());
        }

        let mut names: Vec<String> = Vec::new();
        for export in &exports {
            match export {
                ExportDecl::All => {
                    return Ok(loaded.arrows.keys().cloned().collect());
                }
                ExportDecl::Names(ns) => {
                    for n in ns {
                        if !names.contains(n) {
                            names.push(n.clone());
                        }
                    }
                }
                ExportDecl::NamesFrom(ns, from_path) => {
                    let remote_names = self.public_names(from_path)?;
                    for n in ns {
                        if remote_names.contains(n) && !names.contains(n) {
                            names.push(n.clone());
                        }
                    }
                }
                ExportDecl::AllFrom(from_path) => {
                    let remote_names = self.public_names(from_path)?;
                    for n in remote_names {
                        if !names.contains(&n) {
                            names.push(n);
                        }
                    }
                }
            }
        }
        let _ = imports; // suppress unused warning
        Ok(names)
    }

    /// Return the import declarations of a loaded module (private cache).
    pub fn get_imports(&self, path: &str) -> Result<Vec<ImportDecl>, LoadError> {
        // Ensure the module is loaded (populates meta_cache).
        self.load(path)?;
        Ok(self
            .meta_cache
            .borrow()
            .get(path)
            .map(|(imports, _)| imports.clone())
            .unwrap_or_default())
    }
}

/// Convert a [`RefinementResult`] to a [`Contract`].
fn refinement_to_contract(result: &RefinementResult) -> Contract {
    Contract {
        input: result.input.clone(),
        output: result.output.clone(),
        precondition: Pred::True,
        guarantee: result.guarantee.clone(),
        termination_effect: result.termination.as_ref().map(|m| m.pretty()),
    }
}

// --- Section 5: Term traversal helpers ---

/// Check whether a `Term` contains any `App` node whose callee is an
/// `Arr { op: name }` where `name` is not in `known_names`.
///
/// This is the production equivalent of the PoC's `contains_module_ref`:
/// since `Term` has no `ModuleRef` variant, unresolved `App` nodes to
/// unknown names serve the same role.
pub fn has_unresolved_apps(scheme: &Term, known_names: &HashSet<String>) -> bool {
    let td = TermData::from_term(scheme);
    has_unresolved_apps_td(&td, known_names)
}

/// Data-driven counterpart of [`has_unresolved_apps`], operating on
/// [`TermData`].
fn has_unresolved_apps_td(td: &TermData, known_names: &HashSet<String>) -> bool {
    match td.tag() {
        "Op" => td
            .get_term_list("args")
            .iter()
            .any(|a| has_unresolved_apps_td(a, known_names)),
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            has_unresolved_apps_td(&fst, known_names) || has_unresolved_apps_td(&snd, known_names)
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            has_unresolved_apps_td(&inner, known_names)
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            has_unresolved_apps_td(&l, known_names) || has_unresolved_apps_td(&r, known_names)
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            has_unresolved_apps_td(&body, known_names)
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            if callee.is_tag("Op") {
                if let Some(op) = callee.get_str("name") {
                    if !known_names.contains(op) {
                        return true;
                    }
                }
            }
            has_unresolved_apps_td(&callee, known_names)
                || td
                    .get_term_list("args")
                    .iter()
                    .any(|a| has_unresolved_apps_td(a, known_names))
        }
        // `handle body with target = impl_` (ADR-0076): the handler is
        // eliminated at compile time (impl inlined into body), so the
        // combined graph's unresolved apps are the union of body's and
        // impl's.
        "Handle" => {
            let body = td.get_term("body").expect("Handle: missing body");
            let impl_ = td.get_term("impl_").expect("Handle: missing impl_");
            has_unresolved_apps_td(&body, known_names)
                || has_unresolved_apps_td(&impl_, known_names)
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            has_unresolved_apps_td(&source, known_names)
                || has_unresolved_apps_td(&target, known_names)
        }
        "Arr" | "Child" => false,
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            has_unresolved_apps_td(&fuel, known_names)
                || has_unresolved_apps_td(&predicate, known_names)
                || has_unresolved_apps_td(&step, known_names)
                || has_unresolved_apps_td(&initial, known_names)
        }
        _ => false,
    }
}

/// Check whether a `Term` contains any `Arr { op: "module_ref" }` node
/// (the production representation of a module reference).
pub fn contains_module_ref(scheme: &Term) -> bool {
    let td = TermData::from_term(scheme);
    contains_module_ref_td(&td)
}

/// Data-driven counterpart of [`contains_module_ref`], operating on
/// [`TermData`].
fn contains_module_ref_td(td: &TermData) -> bool {
    match td.tag() {
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            op == "module_ref" || td.get_term_list("args").iter().any(contains_module_ref_td)
        }
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            contains_module_ref_td(&fst) || contains_module_ref_td(&snd)
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            contains_module_ref_td(&inner)
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            contains_module_ref_td(&l) || contains_module_ref_td(&r)
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            contains_module_ref_td(&body)
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            contains_module_ref_td(&callee)
                || td.get_term_list("args").iter().any(contains_module_ref_td)
        }
        // `handle body with target = impl_` (ADR-0076): eliminated at compile
        // time — check both body and impl for module refs.
        "Handle" => {
            let body = td.get_term("body").expect("Handle: missing body");
            let impl_ = td.get_term("impl_").expect("Handle: missing impl_");
            contains_module_ref_td(&body) || contains_module_ref_td(&impl_)
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            contains_module_ref_td(&source) || contains_module_ref_td(&target)
        }
        "Arr" | "Child" => false,
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            contains_module_ref_td(&fuel)
                || contains_module_ref_td(&predicate)
                || contains_module_ref_td(&step)
                || contains_module_ref_td(&initial)
        }
        _ => false,
    }
}

/// Check whether a `Term` contains any `Arr { op: "project" }` node
/// (the production representation of a projection).
pub fn contains_project(scheme: &Term) -> bool {
    let td = TermData::from_term(scheme);
    contains_project_td(&td)
}

/// Data-driven counterpart of [`contains_project`], operating on [`TermData`].
fn contains_project_td(td: &TermData) -> bool {
    match td.tag() {
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            op == "project" || td.get_term_list("args").iter().any(contains_project_td)
        }
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            contains_project_td(&fst) || contains_project_td(&snd)
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            contains_project_td(&inner)
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            contains_project_td(&l) || contains_project_td(&r)
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            contains_project_td(&body)
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            contains_project_td(&callee)
                || td.get_term_list("args").iter().any(contains_project_td)
        }
        // `handle body with target = impl_` (ADR-0076): eliminated at compile
        // time — check both body and impl for projections.
        "Handle" => {
            let body = td.get_term("body").expect("Handle: missing body");
            let impl_ = td.get_term("impl_").expect("Handle: missing impl_");
            contains_project_td(&body) || contains_project_td(&impl_)
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            contains_project_td(&source) || contains_project_td(&target)
        }
        "Arr" | "Child" => false,
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            contains_project_td(&fuel)
                || contains_project_td(&predicate)
                || contains_project_td(&step)
                || contains_project_td(&initial)
        }
        _ => false,
    }
}

/// Build a record `Term` from a map of named arrows. Each field is
/// `arr("field", [arr(name), arrow])`, wrapped in `arr("record", [...])`.
fn build_module_record(arrows: &HashMap<String, Term>) -> Term {
    let fields: Vec<TermData> = arrows
        .iter()
        .map(|(name, scheme)| {
            TermData::op(
                "field".to_string(),
                vec![
                    TermData::arr(Value::String(name.clone())),
                    TermData::from_term(scheme),
                ],
                &ArrSource::Native,
                None,
            )
        })
        .collect();
    TermData::op("record".to_string(), fields, &ArrSource::Native, None)
        .to_term()
        .expect("build_module_record: TermData -> Term conversion failed")
}

/// Data-driven transform: recursively transform a [`TermData`], applying a
/// substitution function to each node bottom-up. The `handle` case is
/// eliminated at compile time via `eliminate_handlers`.
fn transform_scheme_td<F>(td: &TermData, f: &F) -> TermData
where
    F: Fn(&TermData) -> TermData,
{
    let transformed = match td.tag() {
        "Op" => {
            let name = td.get_str("name").unwrap_or("").to_string();
            let source = td.arr_source().unwrap_or(ArrSource::Native);
            let lit = td.get("lit").filter(|v| !v.is_null()).cloned();
            let args: Vec<TermData> = td
                .get_term_list("args")
                .iter()
                .map(|a| transform_scheme_td(a, f))
                .collect();
            TermData::op(name, args, &source, lit)
        }
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            TermData::seq(transform_scheme_td(&fst, f), transform_scheme_td(&snd, f))
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            TermData::first(transform_scheme_td(&inner, f))
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            TermData::choice(transform_scheme_td(&l, f), transform_scheme_td(&r, f))
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            TermData::loop_feedback(transform_scheme_td(&body, f))
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            let args: Vec<TermData> = td
                .get_term_list("args")
                .iter()
                .map(|a| transform_scheme_td(a, f))
                .collect();
            TermData::app(transform_scheme_td(&callee, f), args)
        }
        // `handle body with target = impl_` (ADR-0076): eliminate the handler
        // at compile time, then apply the bottom-up transform to the
        // resulting handler-free graph.
        "Handle" => {
            let eliminated = td
                .to_term()
                .expect("Handle: TermData -> Term conversion failed")
                .eliminate_handlers();
            TermData::from_term(&eliminated)
        }
        "Arr" => {
            let value = td.get("value").cloned().unwrap_or(Value::Null);
            TermData::arr(value)
        }
        "Child" => {
            let role = td.get_str("role").unwrap_or("").to_string();
            TermData::child(role)
        }
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            TermData::iter(
                transform_scheme_td(&fuel, f),
                transform_scheme_td(&predicate, f),
                transform_scheme_td(&step, f),
                transform_scheme_td(&initial, f),
            )
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            let witness_term = td.to_term().expect("Rewrite: TermData -> Term failed");
            if let Term::Rewrite { witness, .. } = witness_term {
                TermData::rewrite(
                    transform_scheme_td(&source, f),
                    transform_scheme_td(&target, f),
                    &witness,
                )
            } else {
                unreachable!()
            }
        }
        _ => td.clone(),
    };
    f(&transformed)
}

// --- Section 6: Strategy A — normalize_compile_time (eager, static IR) ---

/// Normalization Strategy A (ADR-0069 §5): compile-time desugar.
///
/// Resolves the entry module, then walks its arrows: every `App` to an
/// imported name is replaced by a `project` node from an inlined record of
/// the referenced module's public arrows. `module_ref` nodes (if present)
/// are also replaced by inlined records. `project` nodes are kept — Strategy
/// A preserves projections. Recurses transitively with cycle detection.
///
/// Returns a map of the entry module's arrow names to their normalized
/// `Term` graphs (with imports replaced by projections from inlined
/// records).
pub fn normalize_compile_time(
    entry_path: &str,
    loader: &ModuleLoader,
) -> Result<HashMap<String, Term>, LoadError> {
    let mut visited: Vec<String> = Vec::new();
    normalize_compile_time_inner(entry_path, loader, &mut visited)
}

fn normalize_compile_time_inner(
    path: &str,
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
) -> Result<HashMap<String, Term>, LoadError> {
    // Cycle detection.
    if visited.contains(&path.to_string()) {
        let mut cycle: Vec<String> = Vec::new();
        let start_idx = visited.iter().position(|p| p == path).unwrap_or(0);
        for p in &visited[start_idx..] {
            cycle.push(p.clone());
        }
        cycle.push(path.to_string());
        return Err(LoadError::Cycle(cycle));
    }
    visited.push(path.to_string());

    let loaded = loader.load(path)?;
    let imports = loader.get_imports(path)?;

    // Build import_records: local_name → inlined record or project node.
    // Also build path_records: module path → inlined record, so that
    // `module_ref(path)` nodes emitted by `rewrite_imports` can be resolved.
    let mut import_records: HashMap<String, Term> = HashMap::new();
    let mut path_records: HashMap<String, Term> = HashMap::new();
    for import in &imports {
        // Recursively normalize the referenced module.
        let child_arrows = normalize_compile_time_inner(&import.path, loader, visited)?;
        // Add file-stem aliases for single-expression modules (keyed "main")
        // so that `import name from "path"` can find the arrow by file stem.
        let child_arrows = arrows_with_file_stem_alias(&import.path, &child_arrows);
        let record = build_module_record(&child_arrows);
        path_records.insert(import.path.clone(), record.clone());
        for (remote_name, local_name) in &import.names {
            if remote_name == "*" {
                // Whole-module import: bind local_name to the record.
                import_records.insert(local_name.clone(), record.clone());
            } else {
                // Named import: bind local_name to a project from the record.
                let project = TermData::op(
                    "project".to_string(),
                    vec![TermData::from_term(&record)],
                    &ArrSource::Native,
                    Some(Value::String(remote_name.clone())),
                )
                .to_term()
                .expect("normalize: project TermData -> Term failed");
                import_records.insert(local_name.clone(), project);
            }
        }
    }

    // Replace App-to-import and module_ref nodes in each arrow.
    let mut normalized_arrows = HashMap::new();
    for (name, scheme) in &loaded.arrows {
        let normalized = replace_imports_with_projects(scheme, &import_records, &path_records);
        normalized_arrows.insert(name.clone(), normalized);
    }

    visited.pop();
    Ok(normalized_arrows)
}

/// Replace `App { callee: Arr { op: name } }` where `name` is in
/// `import_records` with the corresponding record/project node. Also
/// replaces `Arr { op: "module_ref", lit: Some(path) }` nodes using
/// `path_records` (keyed by module path). `project` nodes are kept —
/// Strategy A preserves projections.
fn replace_imports_with_projects(
    scheme: &Term,
    import_records: &HashMap<String, Term>,
    path_records: &HashMap<String, Term>,
) -> Term {
    let td = TermData::from_term(scheme);
    let import_records_td: HashMap<String, TermData> = import_records
        .iter()
        .map(|(k, v)| (k.clone(), TermData::from_term(v)))
        .collect();
    let path_records_td: HashMap<String, TermData> = path_records
        .iter()
        .map(|(k, v)| (k.clone(), TermData::from_term(v)))
        .collect();
    replace_imports_with_projects_td(&td, &import_records_td, &path_records_td)
        .to_term()
        .expect("replace_imports_with_projects: TermData -> Term failed")
}

/// Data-driven counterpart of [`replace_imports_with_projects`], operating on
/// [`TermData`] and returning [`TermData`].
fn replace_imports_with_projects_td(
    td: &TermData,
    import_records: &HashMap<String, TermData>,
    path_records: &HashMap<String, TermData>,
) -> TermData {
    transform_scheme_td(td, &|node| match node.tag() {
        "App" => {
            let callee = node.get_term("callee").expect("App: missing callee");
            let args = node.get_term_list("args");
            if callee.is_tag("Op") {
                if let Some(op) = callee.get_str("name") {
                    if let Some(replacement) = import_records.get(op) {
                        return replacement.clone();
                    }
                }
            }
            // Not an import reference: keep as-is (already recursed).
            TermData::app(callee, args)
        }
        "Op" if node.get_str("name") == Some("module_ref") => {
            let lit = node.get("lit").filter(|v| !v.is_null()).cloned();
            if let Some(Value::String(path)) = &lit {
                if let Some(replacement) = path_records.get(path) {
                    return replacement.clone();
                }
            }
            // Not found: keep as-is.
            node.clone()
        }
        // Bare reference to a named arrow (no args, no lit): if it matches
        // an import name, replace with the project/record node.
        "Op" => {
            let op = node.get_str("name").unwrap_or("");
            let args = node.get_term_list("args");
            let lit = node.get("lit").filter(|v| !v.is_null());
            if args.is_empty() && lit.is_none() {
                if let Some(replacement) = import_records.get(op) {
                    return replacement.clone();
                }
            }
            node.clone()
        }
        _ => node.clone(),
    })
}

// --- Section 7: Strategy B — flatten_program (whole-program flattening) ---

/// Normalization Strategy B (ADR-0069 §5): whole-program flattening.
///
/// Loads the entry module and all its transitive dependencies, then resolves
/// every `App` node by inlining the referenced arrow's `Term` graph
/// (recursively). `module_ref` nodes are replaced by inlined records, and
/// `project` nodes are resolved to the actual field's arrow. The result is a
/// single `Term` graph with no unresolved `App`, no `module_ref`, and no
/// `project` — one graph, no module boundaries.
///
/// Returns the entry's main arrow (if single-arrow) or a record of all
/// public arrows (if multi-name).
pub fn flatten_program(entry_path: &str, loader: &ModuleLoader) -> Result<Term, LoadError> {
    let mut visited: Vec<String> = Vec::new();
    let mut global_arrows: HashMap<String, Term> = HashMap::new();

    // Load the entry and all transitive dependencies, populating global_arrows.
    load_transitive(entry_path, loader, &mut visited, &mut global_arrows)?;

    // Now resolve all App nodes in the entry's arrows using the global map.
    let entry_loaded = loader.load(entry_path)?;
    let mut resolved_arrows: HashMap<String, Term> = HashMap::new();
    for (name, scheme) in &entry_loaded.arrows {
        let resolved = resolve_all_apps(scheme, &global_arrows, &HashSet::new());
        resolved_arrows.insert(name.clone(), resolved);
    }

    // Return the entry's main arrow or a record of all public arrows.
    if resolved_arrows.len() == 1 {
        Ok(resolved_arrows.into_iter().next().unwrap().1)
    } else if resolved_arrows.is_empty() {
        Ok(TermData::op("id".to_string(), vec![], &ArrSource::Native, None)
            .to_term()
            .expect("flatten_program: id TermData -> Term failed"))
    } else {
        Ok(build_module_record(&resolved_arrows))
    }
}

/// Derive the file stem from a module path (e.g. `"collection/insert"` ->
/// `"insert"`). Used to alias single-expression modules (keyed `"main"`) so
/// bare-name references like `insert` can resolve to `collection/insert.hnl`.
fn module_file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Add a module's arrows to the global map. For single-expression modules
/// (keyed `"main"`), also add an alias under the module path's file stem so
/// bare-name references (e.g. `insert` -> `collection/insert.hnl`) resolve.
fn add_module_arrows(
    global_arrows: &mut HashMap<String, Term>,
    path: &str,
    arrows: &HashMap<String, Term>,
) {
    for (name, scheme) in arrows {
        global_arrows.insert(name.clone(), scheme.clone());
    }
    // File-stem alias for single-expression modules (keyed "main").
    let stem = module_file_stem(path);
    if !arrows.contains_key(&stem) {
        if let Some(scheme) = arrows.get("main") {
            global_arrows.insert(stem, scheme.clone());
        }
    }
}

/// Return a copy of `arrows` with a file-stem alias added for single-expression
/// modules (keyed `"main"`). This ensures that `import name from "path"` can
/// find the arrow by the file stem name (e.g. `import insert from
/// "collection/insert"`) even when the module is a single-expression file
/// whose arrow is keyed `"main"` by the ModuleLoader.
fn arrows_with_file_stem_alias(
    path: &str,
    arrows: &HashMap<String, Term>,
) -> HashMap<String, Term> {
    let mut result = arrows.clone();
    let stem = module_file_stem(path);
    if !result.contains_key(&stem) {
        if let Some(scheme) = result.get("main") {
            result.insert(stem, scheme.clone());
        }
    }
    result
}

/// Derive the parent directory of a module path (e.g. `"collection/sort"` ->
/// `Some("collection")`). Returns `None` for top-level paths (no parent).
fn module_parent_dir(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Build candidate module paths to try when auto-discovering a bare-name
/// `App` callee. The bare name is tried first (e.g. `"insert"`), then the
/// name relative to the current module's directory (e.g. `"collection/insert"`
/// when the current module is `"collection/sort"`). This handles both resolver
/// layouts: root at the file's parent (bare name works) and root at the
/// arrows directory (relative path needed).
fn discovery_candidates(name: &str, current_path: &str) -> Vec<String> {
    let mut candidates = vec![name.to_string()];
    if let Some(dir) = module_parent_dir(current_path) {
        candidates.push(format!("{}/{}", dir, name));
    }
    candidates
}

/// Collect the callee names of all `App { callee: Arr { op: name, .. } }`
/// nodes in a `Term`. Used by `load_transitive` to discover bare-name
/// references to arrows that are not explicitly imported (kit-stdlib files
/// reference other arrows without import declarations).
fn collect_app_callee_names(scheme: &Term, out: &mut HashSet<String>) {
    let td = TermData::from_term(scheme);
    collect_app_callee_names_td(&td, out)
}

/// Data-driven counterpart of [`collect_app_callee_names`], operating on
/// [`TermData`].
fn collect_app_callee_names_td(td: &TermData, out: &mut HashSet<String>) {
    match td.tag() {
        "Op" => {
            for a in td.get_term_list("args") {
                collect_app_callee_names_td(&a, out);
            }
        }
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            collect_app_callee_names_td(&fst, out);
            collect_app_callee_names_td(&snd, out);
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            collect_app_callee_names_td(&inner, out);
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            collect_app_callee_names_td(&l, out);
            collect_app_callee_names_td(&r, out);
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            collect_app_callee_names_td(&body, out);
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            if let Some(op) = callee.get_str("name") {
                out.insert(op.to_string());
            }
            collect_app_callee_names_td(&callee, out);
            for a in td.get_term_list("args") {
                collect_app_callee_names_td(&a, out);
            }
        }
        // `handle body with target = impl_` (ADR-0076): eliminated at compile
        // time — collect callee names from both body and impl.
        "Handle" => {
            let body = td.get_term("body").expect("Handle: missing body");
            let impl_ = td.get_term("impl_").expect("Handle: missing impl_");
            collect_app_callee_names_td(&body, out);
            collect_app_callee_names_td(&impl_, out);
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            collect_app_callee_names_td(&source, out);
            collect_app_callee_names_td(&target, out);
        }
        "Arr" | "Child" | "Const" | "Path" | "Equiv" | "Src" | "Tgt"
        | "RecordProj" | "RecordWith" | "Tag" | "CellId" | "CellCompV"
        | "Whisker" | "Univalence" | "Glue" | "Fibrant" | "JElim"
        | "Refine" | "Cobind" => {}
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            collect_app_callee_names_td(&fuel, out);
            collect_app_callee_names_td(&predicate, out);
            collect_app_callee_names_td(&step, out);
            collect_app_callee_names_td(&initial, out);
        }
        // Unknown/unsupported tags: no App callees to collect.
        _ => {}
    }
}

/// Recursively load a module and all its transitive imports, populating
/// `global_arrows` with every known arrow name → `Term` mapping. Cycle
/// detection via the `visited` stack.
///
/// Beyond explicit `import` declarations, this also auto-discovers bare-name
/// `App` callees (e.g. `sort` calling `insert` with no import) by attempting
/// to load each unresolved name as a module path. Single-expression modules
/// (keyed `"main"`) are also aliased under their file stem so bare-name
/// references resolve.
fn load_transitive(
    path: &str,
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
    global_arrows: &mut HashMap<String, Term>,
) -> Result<(), LoadError> {
    // Cycle detection.
    if visited.contains(&path.to_string()) {
        let mut cycle: Vec<String> = Vec::new();
        let start_idx = visited.iter().position(|p| p == path).unwrap_or(0);
        for p in &visited[start_idx..] {
            cycle.push(p.clone());
        }
        cycle.push(path.to_string());
        return Err(LoadError::Cycle(cycle));
    }

    // Resolve + parse before committing to the visited stack, so a load
    // failure doesn't leave a stale entry that would cause false cycle
    // detection later.
    let loaded = loader.load(path)?;
    let imports = loader.get_imports(path)?;

    visited.push(path.to_string());
    let result = load_transitive_body(path, &loaded, &imports, loader, visited, global_arrows);
    visited.pop();
    result
}

/// Body of [`load_transitive`], run after the module is loaded and `path` is
/// pushed onto the `visited` stack. Separated so the `visited` pop always
/// runs, even on error.
fn load_transitive_body(
    path: &str,
    loaded: &LoadedModule,
    imports: &[ImportDecl],
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
    global_arrows: &mut HashMap<String, Term>,
) -> Result<(), LoadError> {
    // Add this module's arrows to the global map (with file-stem alias).
    add_module_arrows(global_arrows, path, &loaded.arrows);

    // Process imports: load child modules and bind local names.
    for import in imports {
        load_transitive(&import.path, loader, visited, global_arrows)?;
        let child_loaded = loader.load(&import.path)?;
        // Add file-stem aliases for single-expression modules (keyed "main")
        // so that `import name from "path"` can find the arrow by file stem.
        let child_arrows = arrows_with_file_stem_alias(&import.path, &child_loaded.arrows);
        // Bind the module path to a record of the child's arrows, so that
        // `module_ref(path)` nodes emitted by `rewrite_imports` can be
        // resolved by `resolve_all_apps`.
        let record = build_module_record(&child_arrows);
        global_arrows.insert(import.path.clone(), record.clone());
        for (remote_name, local_name) in &import.names {
            if remote_name == "*" {
                // Whole-module import: all child arrows already in global_arrows.
                // Also bind the local alias to a record of the child's arrows.
                global_arrows.insert(local_name.clone(), record.clone());
            } else {
                // Named import: bind local_name to the specific arrow.
                // Check both the raw arrows and the file-stem-aliased arrows.
                if let Some(arrow) = child_arrows.get(remote_name) {
                    global_arrows.insert(local_name.clone(), arrow.clone());
                }
            }
        }
    }

    // Auto-discover bare-name App callees that are not explicitly imported.
    // kit-stdlib files reference other arrows (e.g. `sort` calling `insert`)
    // without import declarations. For each unresolved callee name, try to
    // load it as a module path; if it resolves, recurse into it. Names that
    // don't resolve (e.g. primitive ops like `list.head`) are skipped.
    let mut callee_names: HashSet<String> = HashSet::new();
    for scheme in loaded.arrows.values() {
        collect_app_callee_names(scheme, &mut callee_names);
    }
    for name in &callee_names {
        if !global_arrows.contains_key(name) && !visited.contains(name) {
            // Try candidate module paths: the bare name, then the name
            // relative to the current module's directory (e.g. `insert`
            // -> `collection/insert` when current module is `collection/sort`).
            // This handles both resolver layouts (root at file parent vs.
            // root at arrows directory).
            for candidate in discovery_candidates(name, path) {
                if !visited.contains(&candidate) && loader.load(&candidate).is_ok() {
                    load_transitive(&candidate, loader, visited, global_arrows)?;
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Resolve all `App` nodes in a `Term` by inlining referenced arrows from
/// `known`. Uses `resolving` for cycle detection on names (prevents infinite
/// recursion on self-referential arrows).
fn resolve_all_apps(
    scheme: &Term,
    known: &HashMap<String, Term>,
    resolving: &HashSet<String>,
) -> Term {
    let td = TermData::from_term(scheme);
    resolve_all_apps_td(&td, known, resolving)
        .to_term()
        .expect("resolve_all_apps: TermData -> Term conversion failed")
}

/// Data-driven counterpart of [`resolve_all_apps`], operating on [`TermData`]
/// and returning [`TermData`].
fn resolve_all_apps_td(
    td: &TermData,
    known: &HashMap<String, Term>,
    resolving: &HashSet<String>,
) -> TermData {
    match td.tag() {
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            let source = td.arr_source().unwrap_or(ArrSource::Native);
            let lit = td.get("lit").filter(|v| !v.is_null()).cloned();
            let args = td.get_term_list("args");
            // Handle module_ref nodes: replace with inlined record if known.
            if op == "module_ref" {
                if let Some(Value::String(path)) = &lit {
                    if let Some(record) = known.get(path) {
                        if !resolving.contains(path) {
                            let mut new_resolving = resolving.clone();
                            new_resolving.insert(path.clone());
                            let record_td = TermData::from_term(record);
                            return resolve_all_apps_td(&record_td, known, &new_resolving);
                        }
                    }
                }
            }
            // Handle project nodes: resolve the field from the record.
            if op == "project" && args.len() == 1 {
                if let Some(Value::String(field)) = &lit {
                    let resolved_record = resolve_all_apps_td(&args[0], known, resolving);
                    if let Some(target) = lookup_field_in_record_td(&resolved_record, field) {
                        if !resolving.contains(field) {
                            let mut new_resolving = resolving.clone();
                            new_resolving.insert(field.clone());
                            return resolve_all_apps_td(&target, known, &new_resolving);
                        }
                    }
                }
            }
            // Bare reference to a named arrow (no args, no lit): inline if known.
            if args.is_empty() && lit.is_none() {
                if let Some(target) = known.get(op) {
                    if !resolving.contains(op) {
                        let mut new_resolving = resolving.clone();
                        new_resolving.insert(op.to_string());
                        let target_td = TermData::from_term(target);
                        return resolve_all_apps_td(&target_td, known, &new_resolving);
                    }
                }
            }
            let new_args: Vec<TermData> = args
                .iter()
                .map(|a| resolve_all_apps_td(a, known, resolving))
                .collect();
            TermData::op(op.to_string(), new_args, &source, lit)
        }
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            TermData::seq(
                resolve_all_apps_td(&fst, known, resolving),
                resolve_all_apps_td(&snd, known, resolving),
            )
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            TermData::first(resolve_all_apps_td(&inner, known, resolving))
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            TermData::choice(
                resolve_all_apps_td(&l, known, resolving),
                resolve_all_apps_td(&r, known, resolving),
            )
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            TermData::loop_feedback(resolve_all_apps_td(&body, known, resolving))
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            let args = td.get_term_list("args");
            // First, try to resolve the callee as a known bare-name reference.
            if callee.is_tag("Op") {
                if let Some(op) = callee.get_str("name") {
                    if let Some(target) = known.get(op) {
                        if !resolving.contains(op) {
                            let mut new_resolving = resolving.clone();
                            new_resolving.insert(op.to_string());
                            let target_td = TermData::from_term(target);
                            return resolve_all_apps_td(&target_td, known, &new_resolving);
                        }
                    }
                }
            }
            // Check if the callee is an import reference (project/module_ref
            // node emitted by `rewrite_imports`). If so, resolve it and inline
            // the result — replacing the App with the imported arrow's graph.
            // This matches `resolve_apps` which inlines
            // `App { callee: arr("name"), args: [] }` by replacing it with
            // the referenced graph.
            let is_import_ref = is_import_reference_td(&callee);
            let resolved_callee = resolve_all_apps_td(&callee, known, resolving);
            if is_import_ref && args.is_empty() && !is_import_reference_td(&resolved_callee) {
                // The import reference resolved to a concrete graph — inline it.
                return resolved_callee;
            }
            let new_args: Vec<TermData> = args
                .iter()
                .map(|a| resolve_all_apps_td(a, known, resolving))
                .collect();
            TermData::app(resolved_callee, new_args)
        }
        // `handle body with target = impl_` (ADR-0076): eliminate the handler
        // at compile time, then resolve apps in the resulting handler-free
        // graph.
        "Handle" => {
            let eliminated = td
                .to_term()
                .expect("Handle: TermData -> Term conversion failed")
                .eliminate_handlers();
            let eliminated_td = TermData::from_term(&eliminated);
            resolve_all_apps_td(&eliminated_td, known, resolving)
        }
        "Arr" => {
            let value = td.get("value").cloned().unwrap_or(Value::Null);
            TermData::arr(value)
        }
        "Child" => {
            let role = td.get_str("role").unwrap_or("").to_string();
            TermData::child(role)
        }
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            TermData::iter(
                resolve_all_apps_td(&fuel, known, resolving),
                resolve_all_apps_td(&predicate, known, resolving),
                resolve_all_apps_td(&step, known, resolving),
                resolve_all_apps_td(&initial, known, resolving),
            )
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            let witness_term = td.to_term().expect("Rewrite: TermData -> Term failed");
            if let Term::Rewrite { witness, .. } = witness_term {
                TermData::rewrite(
                    resolve_all_apps_td(&source, known, resolving),
                    resolve_all_apps_td(&target, known, resolving),
                    &witness,
                )
            } else {
                unreachable!()
            }
        }
        _ => {
            // Leaf nodes and HoTT-specific terms that don't contain App
            // callees to resolve: preserve as-is.
            td.clone()
        }
    }
}

/// Check whether a `TermData` is an import reference node — a `project` or
/// `module_ref` node emitted by `rewrite_imports`. Used by
/// `resolve_all_apps_td` to determine whether an `App`'s callee is an
/// import-rewritten reference that should be inlined after resolution.
fn is_import_reference_td(td: &TermData) -> bool {
    if td.is_tag("Op") {
        let op = td.get_str("name").unwrap_or("");
        op == "project" || op == "module_ref"
    } else {
        false
    }
}

/// Look up a field by name in an inlined module record. The record is
/// expected to be `arr("record", [arr("field", [arr("const", lit: name), value])...])`.
/// Returns the value `TermData` if found.
fn lookup_field_in_record_td(td: &TermData, field: &str) -> Option<TermData> {
    if td.is_tag("Op") && td.get_str("name") == Some("record") {
        for f in td.get_term_list("args") {
            if f.is_tag("Op") && f.get_str("name") == Some("field") {
                let fargs = f.get_term_list("args");
                if fargs.len() == 2 {
                    if fargs[0].is_tag("Arr") {
                        if let Some(name) = fargs[0].get("value").and_then(|v| v.as_str()) {
                            if name == field {
                                return Some(fargs[1].clone());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
