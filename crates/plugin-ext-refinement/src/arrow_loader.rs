//! File loader: reads `.hnl` arrow files, parses them into `Scheme` IR,
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
//! and inlining its `Scheme` graph -- matching the hand-built graphs in
//! `e2e_tests.rs` where `sort` inlines `insert`'s full graph.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use handl_language::{ArrSource, Scheme, Value};

use crate::arrow_parser::{
    parse_arrow_file, parse_arrow_module, ExportDecl, ImportDecl, ParsedFile,
};
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::{Contract, Pred, RefinementResult};

// ============================================================================
// Public API
// ============================================================================

/// Load a single `.hnl` arrow file and return its `Scheme` graph.
///
/// For single-expression files, returns the arrow directly.
/// For multi-name files, returns the first named arrow (or the whole record
/// if there's only one).
pub fn load_arrow_file(path: &str) -> Result<Scheme, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("failed to read {}: {}", path, e))?;
    let parsed = parse_arrow_file(&content)?;

    match parsed {
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
pub fn load_arrow_file_named(path: &str) -> Result<HashMap<String, Scheme>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("failed to read {}: {}", path, e))?;
    let parsed = parse_arrow_file(&content)?;

    match parsed {
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

/// Load and infer: parse `.hnl` -> `Scheme` -> resolve references -> `infer_refinement`.
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
/// name -> `Scheme` graph map.
///
/// The key is the file stem (e.g. `insert.hnl` -> `"insert"`).
pub fn load_all_arrows(dir: &str) -> Result<HashMap<String, Scheme>, String> {
    let mut map = HashMap::new();
    walk_dir(dir, &mut map)?;
    Ok(map)
}

/// Resolve `App` nodes in a `Scheme` by inlining referenced graphs.
///
/// When an `App { callee: arr("name") }` is found and `"name"` exists in the
/// graph map, the App is replaced with the referenced graph (recursively
/// resolved). This matches the hand-built graphs in `e2e_tests.rs` where
/// `sort` inlines `insert`'s full graph.
pub fn resolve_apps(scheme: &Scheme, graph_map: &HashMap<String, Scheme>) -> Scheme {
    resolve_apps_inner(scheme, graph_map, 0)
}

/// Find the arrows root directory from a file path.
///
/// Looks for `/arrows/` in the path and returns everything up to and
/// including it. Returns `None` if the path doesn't contain `/arrows/`.
fn find_arrows_root(path: &str) -> Option<String> {
    if let Some(idx) = path.find("/arrows/") {
        Some(path[..idx + "/arrows/".len()].to_string())
    } else if path.ends_with("/arrows") {
        Some(path.to_string())
    } else {
        None
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Recursively walk a directory and load all `.hnl` files into the map.
///
/// Both single-expression files and multi-name files are loaded. Files that
/// fail to parse are silently skipped (the parser is minimal and may not
/// handle all syntax).
fn walk_dir(dir: &str, map: &mut HashMap<String, Scheme>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("failed to read directory {}: {}", dir, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
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
            // Parse -- skip files that fail (the parser is minimal)
            if let Ok(parsed) = parse_arrow_file(&content) {
                match parsed {
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
fn resolve_apps_inner(
    scheme: &Scheme,
    graph_map: &HashMap<String, Scheme>,
    depth: usize,
) -> Scheme {
    // Depth limit to prevent infinite recursion on circular references
    if depth > 16 {
        return scheme.clone();
    }

    match scheme {
        Scheme::Arr { .. } => scheme.clone(),

        Scheme::Seq { fst, snd } => Scheme::Seq {
            fst: Box::new(resolve_apps_inner(fst, graph_map, depth)),
            snd: Box::new(resolve_apps_inner(snd, graph_map, depth)),
        },

        Scheme::First { inner } => Scheme::First {
            inner: Box::new(resolve_apps_inner(inner, graph_map, depth)),
        },

        Scheme::Choice { l, r } => Scheme::Choice {
            l: Box::new(resolve_apps_inner(l, graph_map, depth)),
            r: Box::new(resolve_apps_inner(r, graph_map, depth)),
        },

        Scheme::Loop { body } => Scheme::Loop {
            body: Box::new(resolve_apps_inner(body, graph_map, depth)),
        },

        Scheme::App { callee, args } => {
            // Check if the callee is a reference to a named arrow
            if let Scheme::Arr { op, .. } = callee.as_ref() {
                if let Some(graph) = graph_map.get(op) {
                    // Inline the referenced graph (recursively resolved)
                    return resolve_apps_inner(graph, graph_map, depth + 1);
                }
            }
            // If not found, keep the App as-is (inference returns True for App)
            Scheme::App {
                callee: Box::new(resolve_apps_inner(callee, graph_map, depth)),
                args: args
                    .iter()
                    .map(|a| resolve_apps_inner(a, graph_map, depth))
                    .collect(),
            }
        }
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
    /// The named arrows in this module (path -> Scheme).
    pub arrows: HashMap<String, Scheme>,
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
    pub graph_cache: RefCell<HashMap<String, HashMap<String, Scheme>>>,
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

        // Convert ParsedFile to HashMap<String, Scheme>.
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
    pub fn load_arrow(&self, path: &str, name: &str) -> Result<Scheme, LoadError> {
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

// --- Section 5: Scheme traversal helpers ---

/// Check whether a `Scheme` contains any `App` node whose callee is an
/// `Arr { op: name }` where `name` is not in `known_names`.
///
/// This is the production equivalent of the PoC's `contains_module_ref`:
/// since `Scheme` has no `ModuleRef` variant, unresolved `App` nodes to
/// unknown names serve the same role.
pub fn has_unresolved_apps(scheme: &Scheme, known_names: &HashSet<String>) -> bool {
    match scheme {
        Scheme::Arr { args, .. } => {
            args.iter().any(|a| has_unresolved_apps(a, known_names))
        }
        Scheme::Seq { fst, snd } => {
            has_unresolved_apps(fst, known_names) || has_unresolved_apps(snd, known_names)
        }
        Scheme::First { inner } => has_unresolved_apps(inner, known_names),
        Scheme::Choice { l, r } => {
            has_unresolved_apps(l, known_names) || has_unresolved_apps(r, known_names)
        }
        Scheme::Loop { body } => has_unresolved_apps(body, known_names),
        Scheme::App { callee, args } => {
            if let Scheme::Arr { op, .. } = callee.as_ref() {
                if !known_names.contains(op) {
                    return true;
                }
            }
            has_unresolved_apps(callee, known_names)
                || args.iter().any(|a| has_unresolved_apps(a, known_names))
        }
    }
}

/// Check whether a `Scheme` contains any `Arr { op: "module_ref" }` node
/// (the production representation of a module reference).
pub fn contains_module_ref(scheme: &Scheme) -> bool {
    match scheme {
        Scheme::Arr { op, args, .. } => {
            op == "module_ref" || args.iter().any(contains_module_ref)
        }
        Scheme::Seq { fst, snd } => {
            contains_module_ref(fst) || contains_module_ref(snd)
        }
        Scheme::First { inner } => contains_module_ref(inner),
        Scheme::Choice { l, r } => contains_module_ref(l) || contains_module_ref(r),
        Scheme::Loop { body } => contains_module_ref(body),
        Scheme::App { callee, args } => {
            contains_module_ref(callee) || args.iter().any(contains_module_ref)
        }
    }
}

/// Check whether a `Scheme` contains any `Arr { op: "project" }` node
/// (the production representation of a projection).
pub fn contains_project(scheme: &Scheme) -> bool {
    match scheme {
        Scheme::Arr { op, args, .. } => {
            op == "project" || args.iter().any(contains_project)
        }
        Scheme::Seq { fst, snd } => contains_project(fst) || contains_project(snd),
        Scheme::First { inner } => contains_project(inner),
        Scheme::Choice { l, r } => contains_project(l) || contains_project(r),
        Scheme::Loop { body } => contains_project(body),
        Scheme::App { callee, args } => {
            contains_project(callee) || args.iter().any(contains_project)
        }
    }
}

/// Build a record `Scheme` from a map of named arrows. Each field is
/// `arr("field", [arr("const", lit: name), arrow])`, wrapped in
/// `arr("record", [...])`.
fn build_module_record(arrows: &HashMap<String, Scheme>) -> Scheme {
    let fields: Vec<Scheme> = arrows
        .iter()
        .map(|(name, scheme)| {
            Scheme::Arr {
                op: "field".to_string(),
                args: vec![
                    Scheme::Arr {
                        op: "const".to_string(),
                        args: vec![],
                        source: ArrSource::Native,
                        lit: Some(Value::String(name.clone())),
                    },
                    scheme.clone(),
                ],
                source: ArrSource::Native,
                lit: None,
            }
        })
        .collect();
    Scheme::Arr {
        op: "record".to_string(),
        args: fields,
        source: ArrSource::Native,
        lit: None,
    }
}

/// Recursively transform a `Scheme`, applying a substitution function to each
/// node bottom-up.
fn transform_scheme<F>(scheme: &Scheme, f: &F) -> Scheme
where
    F: Fn(&Scheme) -> Scheme,
{
    let transformed = match scheme {
        Scheme::Arr {
            op,
            args,
            source,
            lit,
        } => Scheme::Arr {
            op: op.clone(),
            args: args.iter().map(|a| transform_scheme(a, f)).collect(),
            source: source.clone(),
            lit: lit.clone(),
        },
        Scheme::Seq { fst, snd } => Scheme::Seq {
            fst: Box::new(transform_scheme(fst, f)),
            snd: Box::new(transform_scheme(snd, f)),
        },
        Scheme::First { inner } => Scheme::First {
            inner: Box::new(transform_scheme(inner, f)),
        },
        Scheme::Choice { l, r } => Scheme::Choice {
            l: Box::new(transform_scheme(l, f)),
            r: Box::new(transform_scheme(r, f)),
        },
        Scheme::Loop { body } => Scheme::Loop {
            body: Box::new(transform_scheme(body, f)),
        },
        Scheme::App { callee, args } => Scheme::App {
            callee: Box::new(transform_scheme(callee, f)),
            args: args.iter().map(|a| transform_scheme(a, f)).collect(),
        },
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
/// `Scheme` graphs (with imports replaced by projections from inlined
/// records).
pub fn normalize_compile_time(
    entry_path: &str,
    loader: &ModuleLoader,
) -> Result<HashMap<String, Scheme>, LoadError> {
    let mut visited: Vec<String> = Vec::new();
    normalize_compile_time_inner(entry_path, loader, &mut visited)
}

fn normalize_compile_time_inner(
    path: &str,
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
) -> Result<HashMap<String, Scheme>, LoadError> {
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
    let mut import_records: HashMap<String, Scheme> = HashMap::new();
    let mut path_records: HashMap<String, Scheme> = HashMap::new();
    for import in &imports {
        // Recursively normalize the referenced module.
        let child_arrows = normalize_compile_time_inner(&import.path, loader, visited)?;
        let record = build_module_record(&child_arrows);
        path_records.insert(import.path.clone(), record.clone());
        for (remote_name, local_name) in &import.names {
            if remote_name == "*" {
                // Whole-module import: bind local_name to the record.
                import_records.insert(local_name.clone(), record.clone());
            } else {
                // Named import: bind local_name to a project from the record.
                let project = Scheme::Arr {
                    op: "project".to_string(),
                    args: vec![record.clone()],
                    source: ArrSource::Native,
                    lit: Some(Value::String(remote_name.clone())),
                };
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
    scheme: &Scheme,
    import_records: &HashMap<String, Scheme>,
    path_records: &HashMap<String, Scheme>,
) -> Scheme {
    transform_scheme(scheme, &|node| match node {
        Scheme::App { callee, args } => {
            if let Scheme::Arr { op, .. } = callee.as_ref() {
                if let Some(replacement) = import_records.get(op) {
                    return replacement.clone();
                }
            }
            // Not an import reference: keep as-is (already recursed).
            Scheme::App {
                callee: callee.clone(),
                args: args.clone(),
            }
        }
        Scheme::Arr { op, args, source, lit } if op == "module_ref" => {
            if let Some(Value::String(path)) = lit {
                if let Some(replacement) = path_records.get(path) {
                    return replacement.clone();
                }
            }
            Scheme::Arr {
                op: op.clone(),
                args: args.clone(),
                source: source.clone(),
                lit: lit.clone(),
            }
        }
        // Bare reference to a named arrow (no args, no lit): if it matches
        // an import name, replace with the project/record node.
        Scheme::Arr { op, args, source, lit }
            if args.is_empty() && lit.is_none() =>
        {
            if let Some(replacement) = import_records.get(op) {
                return replacement.clone();
            }
            Scheme::Arr {
                op: op.clone(),
                args: args.clone(),
                source: source.clone(),
                lit: lit.clone(),
            }
        }
        other => other.clone(),
    })
}

// --- Section 7: Strategy B — flatten_program (whole-program flattening) ---

/// Normalization Strategy B (ADR-0069 §5): whole-program flattening.
///
/// Loads the entry module and all its transitive dependencies, then resolves
/// every `App` node by inlining the referenced arrow's `Scheme` graph
/// (recursively). `module_ref` nodes are replaced by inlined records, and
/// `project` nodes are resolved to the actual field's arrow. The result is a
/// single `Scheme` graph with no unresolved `App`, no `module_ref`, and no
/// `project` — one graph, no module boundaries.
///
/// Returns the entry's main arrow (if single-arrow) or a record of all
/// public arrows (if multi-name).
pub fn flatten_program(entry_path: &str, loader: &ModuleLoader) -> Result<Scheme, LoadError> {
    let mut visited: Vec<String> = Vec::new();
    let mut global_arrows: HashMap<String, Scheme> = HashMap::new();

    // Load the entry and all transitive dependencies, populating global_arrows.
    load_transitive(entry_path, loader, &mut visited, &mut global_arrows)?;

    // Now resolve all App nodes in the entry's arrows using the global map.
    let entry_loaded = loader.load(entry_path)?;
    let mut resolved_arrows: HashMap<String, Scheme> = HashMap::new();
    for (name, scheme) in &entry_loaded.arrows {
        let resolved = resolve_all_apps(scheme, &global_arrows, &HashSet::new());
        resolved_arrows.insert(name.clone(), resolved);
    }

    // Return the entry's main arrow or a record of all public arrows.
    if resolved_arrows.len() == 1 {
        Ok(resolved_arrows.into_iter().next().unwrap().1)
    } else if resolved_arrows.is_empty() {
        Ok(Scheme::arr_native("id", vec![]))
    } else {
        Ok(build_module_record(&resolved_arrows))
    }
}

/// Recursively load a module and all its transitive imports, populating
/// `global_arrows` with every known arrow name → `Scheme` mapping. Cycle
/// detection via the `visited` stack.
fn load_transitive(
    path: &str,
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
    global_arrows: &mut HashMap<String, Scheme>,
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
    visited.push(path.to_string());

    let loaded = loader.load(path)?;
    let imports = loader.get_imports(path)?;

    // Add this module's arrows to the global map.
    for (name, scheme) in &loaded.arrows {
        global_arrows.insert(name.clone(), scheme.clone());
    }

    // Process imports: load child modules and bind local names.
    for import in &imports {
        load_transitive(&import.path, loader, visited, global_arrows)?;
        let child_loaded = loader.load(&import.path)?;
        // Bind the module path to a record of the child's arrows, so that
        // `module_ref(path)` nodes emitted by `rewrite_imports` can be
        // resolved by `resolve_all_apps`.
        let record = build_module_record(&child_loaded.arrows);
        global_arrows.insert(import.path.clone(), record.clone());
        for (remote_name, local_name) in &import.names {
            if remote_name == "*" {
                // Whole-module import: all child arrows already in global_arrows.
                // Also bind the local alias to a record of the child's arrows.
                global_arrows.insert(local_name.clone(), record.clone());
            } else {
                // Named import: bind local_name to the specific arrow.
                if let Some(arrow) = child_loaded.arrows.get(remote_name) {
                    global_arrows.insert(local_name.clone(), arrow.clone());
                }
            }
        }
    }

    visited.pop();
    Ok(())
}

/// Resolve all `App` nodes in a `Scheme` by inlining referenced arrows from
/// `known`. Uses `resolving` for cycle detection on names (prevents infinite
/// recursion on self-referential arrows).
fn resolve_all_apps(
    scheme: &Scheme,
    known: &HashMap<String, Scheme>,
    resolving: &HashSet<String>,
) -> Scheme {
    match scheme {
        Scheme::Arr { op, args, source, lit } => {
            // Handle module_ref nodes: replace with inlined record if known.
            if op == "module_ref" {
                if let Some(Value::String(path)) = lit {
                    if let Some(record) = known.get(path) {
                        if !resolving.contains(path) {
                            let mut new_resolving = resolving.clone();
                            new_resolving.insert(path.clone());
                            return resolve_all_apps(record, known, &new_resolving);
                        }
                    }
                }
            }
            // Handle project nodes: resolve the field from the record.
            if op == "project" && args.len() == 1 {
                if let Some(Value::String(field)) = lit {
                    let resolved_record = resolve_all_apps(&args[0], known, resolving);
                    if let Some(target) = lookup_field_in_record(&resolved_record, field) {
                        if !resolving.contains(field) {
                            let mut new_resolving = resolving.clone();
                            new_resolving.insert(field.clone());
                            return resolve_all_apps(&target, known, &new_resolving);
                        }
                    }
                }
            }
            // Bare reference to a named arrow (no args, no lit): inline if known.
            if args.is_empty() && lit.is_none() {
                if let Some(target) = known.get(op) {
                    if !resolving.contains(op) {
                        let mut new_resolving = resolving.clone();
                        new_resolving.insert(op.clone());
                        return resolve_all_apps(target, known, &new_resolving);
                    }
                }
            }
            Scheme::Arr {
                op: op.clone(),
                args: args.iter().map(|a| resolve_all_apps(a, known, resolving)).collect(),
                source: source.clone(),
                lit: lit.clone(),
            }
        }
        Scheme::Seq { fst, snd } => Scheme::Seq {
            fst: Box::new(resolve_all_apps(fst, known, resolving)),
            snd: Box::new(resolve_all_apps(snd, known, resolving)),
        },
        Scheme::First { inner } => Scheme::First {
            inner: Box::new(resolve_all_apps(inner, known, resolving)),
        },
        Scheme::Choice { l, r } => Scheme::Choice {
            l: Box::new(resolve_all_apps(l, known, resolving)),
            r: Box::new(resolve_all_apps(r, known, resolving)),
        },
        Scheme::Loop { body } => Scheme::Loop {
            body: Box::new(resolve_all_apps(body, known, resolving)),
        },
        Scheme::App { callee, args } => {
            if let Scheme::Arr { op, .. } = callee.as_ref() {
                if let Some(target) = known.get(op) {
                    if !resolving.contains(op) {
                        let mut new_resolving = resolving.clone();
                        new_resolving.insert(op.clone());
                        return resolve_all_apps(target, known, &new_resolving);
                    }
                }
            }
            Scheme::App {
                callee: Box::new(resolve_all_apps(callee, known, resolving)),
                args: args.iter().map(|a| resolve_all_apps(a, known, resolving)).collect(),
            }
        }
    }
}

/// Look up a field by name in an inlined module record. The record is
/// expected to be `arr("record", [arr("field", [arr("const", lit: name), value])...])`.
/// Returns the value `Scheme` if found.
fn lookup_field_in_record(record: &Scheme, field: &str) -> Option<Scheme> {
    if let Scheme::Arr { op, args, .. } = record {
        if op == "record" {
            for f in args {
                if let Scheme::Arr { op: fop, args: fargs, .. } = f {
                    if fop == "field" && fargs.len() == 2 {
                        if let Scheme::Arr { op: cop, lit: Some(Value::String(name)), .. } = &fargs[0] {
                            if cop == "const" && name == field {
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
