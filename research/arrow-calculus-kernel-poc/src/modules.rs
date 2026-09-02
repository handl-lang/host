// handl v0.1 — Pluggable Module Resolver and Normalization Strategies
// ADR-0069 §4 (two caches), §5 (three strategies), §7 (multi-file).
//
// This is a standalone test binary in the PoC style (fn main() with
// assertions). It is std-only (no external dependencies). It wires to the
// existing parser/desugar via #[path = "v0_1_surface.rs"] and implements:
//
//   - ResolvedModule + ResolveError
//   - ModuleResolver trait + FileResolver / MemoryResolver / ChainResolver / CachingResolver
//   - Contract (minimal stub for the ADR-0070 refinement engine)
//   - ModuleLoader (two caches: graph + contract, Strategy C)
//   - normalize_compile_time  (Strategy A — eager, static IR)
//   - flatten_program         (Strategy B — whole-program flattening)
//
// See ADR-0069 §5 for the three strategies and §4 for the two caches.

// === Section 1: v0_1_surface module wiring ===

#[path = "v0_1_surface.rs"]
mod v0_1_surface;

use v0_1_surface::{Kernel, parse, desugar};
use v0_1_surface::desugar::{DesugaredProgram, ExportSet};
use v0_1_surface::ast::Lit;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

// === Section 2: ResolvedModule and errors ===

/// A resolved module: its logical path, source text, and origin label
/// (e.g. "file", "memory", "virtual"). Produced by a `ModuleResolver`.
/// (ADR-0069 §4 — the loader consumes resolved modules to populate caches.)
#[derive(Debug, Clone)]
pub struct ResolvedModule {
    pub path: String,
    pub source: String,
    pub origin: String,
}

/// Error kind for module resolution failures.
#[derive(Debug, Clone)]
pub enum ResolveErrorKind {
    /// The path was not found by any resolver in the chain.
    NotFound,
    /// An I/O error occurred while reading the module source.
    IoError(String),
    /// The host denied the capability required to resolve this path
    /// (ADR-0046 — capability discipline).
    Disallowed,
}

/// A resolution error: the path that failed plus a kind.
#[derive(Debug, Clone)]
pub struct ResolveError {
    pub path: String,
    pub kind: ResolveErrorKind,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.kind {
            ResolveErrorKind::NotFound => {
                write!(f, "module not found: {}", self.path)
            }
            ResolveErrorKind::IoError(msg) => {
                write!(f, "io error resolving {}: {}", self.path, msg)
            }
            ResolveErrorKind::Disallowed => {
                write!(
                    f,
                    "capability denied: cannot resolve {} (ADR-0046)",
                    self.path
                )
            }
        }
    }
}

// === Section 3: ModuleResolver trait + built-in resolvers ===

/// The module resolver trait. Resolution is a host capability boundary
/// (ADR-0046, ADR-0028, ADR-0035): the resolver set determines which paths
/// are importable. `resolve(&self)` is immutable; caches use interior
/// mutability (`RefCell`).
///
/// See ADR-0069 §4 and ARROW_DESUGARING_V0_1.md §12.6.
pub trait ModuleResolver {
    /// Resolve a module path to its source text.
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError>;
    /// A short label for where this resolver finds modules ("file", "memory",
    /// "chain", "cache").
    fn origin(&self) -> &'static str;
}

/// `FileResolver` — resolves `path` to `<root>/<path>.hnl` (appending `.hnl`
/// if the path has no extension). Reads the file from disk. origin = "file".
/// This is the default resolver for compiled builds (ADR-0069 §7).
pub struct FileResolver {
    root: PathBuf,
}

impl FileResolver {
    pub fn new(root: PathBuf) -> Self {
        FileResolver { root }
    }
}

impl ModuleResolver for FileResolver {
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError> {
        // Append .hnl if the path has no extension.
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
/// source. Used for REPL, tests, and virtual environments. origin = "memory".
pub struct MemoryResolver {
    modules: HashMap<String, String>,
}

impl MemoryResolver {
    pub fn new() -> Self {
        MemoryResolver {
            modules: HashMap::new(),
        }
    }

    pub fn insert(&mut self, path: &str, source: &str) {
        self.modules.insert(path.to_string(), source.to_string());
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

/// `ChainResolver` — tries each resolver in order, returning the first Ok.
/// If all fail, returns the last error (or NotFound if empty). origin = "chain".
pub struct ChainResolver {
    resolvers: Vec<Box<dyn ModuleResolver>>,
}

impl ChainResolver {
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
/// path. Uses `RefCell` since `resolve(&self)` is immutable. origin delegates
/// to the inner resolver.
pub struct CachingResolver {
    inner: Box<dyn ModuleResolver>,
    cache: RefCell<HashMap<String, ResolvedModule>>,
}

impl CachingResolver {
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

// === Section 4: Contract (minimal stub, ADR-0069 §4 / ADR-0070) ===

/// A minimal representative contract. The real contract system is the
/// refinement inference engine (ADR-0070); this stub captures a signature
/// string and a list of refinement predicates for demonstration purposes.
/// Both caches in `ModuleLoader` (§4) are keyed by path; the contract cache
/// stores these.
#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    /// A best-effort arrow type signature, e.g. "List α ~> List α".
    pub signature: String,
    /// Refinement predicates, e.g. ["Sorted(output)"].
    pub refinements: Vec<String>,
}

/// Infer a placeholder contract from a desugared program. This is a STUB for
/// the real ADR-0070 refinement engine. The heuristic:
/// - More than one binding => "Unit ~> Record(<names>)".
/// - Exactly one binding => "? ~> ?" (no real type engine).
/// - Zero bindings => "Unit ~> Unit".
/// - If any top-level kernel contains a `Loop`, push "LoopInvariant(output)".
///
/// Documented as a placeholder — do not rely on these signatures for real
/// verification.
pub fn infer_contract(desugared: &DesugaredProgram) -> Contract {
    let signature = if desugared.bindings.len() > 1 {
        let names: Vec<String> = desugared.bindings.iter().map(|(n, _)| n.clone()).collect();
        format!("Unit ~> Record({})", names.join(", "))
    } else if desugared.bindings.len() == 1 {
        "? ~> ?".to_string()
    } else {
        "Unit ~> Unit".to_string()
    };

    let mut refinements: Vec<String> = Vec::new();
    for (_, k) in &desugared.bindings {
        if contains_loop(k) {
            refinements.push("LoopInvariant(output)".to_string());
            break;
        }
    }

    Contract {
        signature,
        refinements,
    }
}

/// Helper: does this kernel tree contain a `Loop` node?
fn contains_loop(k: &Kernel) -> bool {
    match k {
        Kernel::Loop { body } => true || contains_loop(body),
        Kernel::Seq { fst, snd } => contains_loop(fst) || contains_loop(snd),
        Kernel::First { inner } => contains_loop(inner),
        Kernel::Choice { l, r } => contains_loop(l) || contains_loop(r),
        Kernel::App { callee, args } => {
            contains_loop(callee) || args.iter().any(contains_loop)
        }
        Kernel::Arr { args, .. } => args.iter().any(contains_loop),
        Kernel::Project { record, .. } => contains_loop(record),
        Kernel::Var(_) | Kernel::Const(_) | Kernel::ModuleRef { .. } => false,
    }
}

// === Section 5: LoadError ===

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

// === Section 6: LoadedModule and ModuleLoader (ADR-0069 §4, Strategy C) ===

/// The result of loading a module: its desugared graph and inferred contract,
/// cloned from the loader's caches.
#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub graph: DesugaredProgram,
    pub contract: Contract,
}

/// `ModuleLoader` maintains two caches keyed by module path (ADR-0069 §4):
/// - **Graph cache**: `path → DesugaredProgram` (the parsed arrow graph).
/// - **Contract cache**: `path → Contract` (the inferred contract).
///
/// On first load, the loader resolves the source, parses it, desugars it,
/// infers its contract, and stores both in the caches. Subsequent loads
/// return cached clones. This is Strategy C (runtime module loading, lazy +
/// cached) — see ADR-0069 §5.
pub struct ModuleLoader {
    resolver: Box<dyn ModuleResolver>,
    graph_cache: RefCell<HashMap<String, DesugaredProgram>>,
    contract_cache: RefCell<HashMap<String, Contract>>,
}

impl ModuleLoader {
    pub fn new(resolver: Box<dyn ModuleResolver>) -> Self {
        ModuleLoader {
            resolver,
            graph_cache: RefCell::new(HashMap::new()),
            contract_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Load a module by path. On first access: resolve → parse → desugar →
    /// infer contract → cache both. On subsequent access: return cached
    /// clones. Returns `LoadedModule` (graph + contract).
    pub fn load(&self, path: &str) -> Result<LoadedModule, LoadError> {
        // Cache hit?
        if let Some(graph) = self.graph_cache.borrow().get(path) {
            let contract = self
                .contract_cache
                .borrow()
                .get(path)
                .cloned()
                .unwrap_or_else(|| Contract {
                    signature: "Unit ~> Unit".to_string(),
                    refinements: vec![],
                });
            return Ok(LoadedModule {
                graph: graph.clone(),
                contract,
            });
        }

        // Resolve the source.
        let resolved = self.resolver.resolve(path).map_err(LoadError::Resolve)?;

        // Parse.
        let program = parse(&resolved.source).map_err(LoadError::Parse)?;

        // Desugar.
        let desugared = desugar(&program);

        // Infer contract (stub).
        let contract = infer_contract(&desugared);

        // Cache both.
        self.graph_cache
            .borrow_mut()
            .insert(path.to_string(), desugared.clone());
        self.contract_cache
            .borrow_mut()
            .insert(path.to_string(), contract.clone());

        Ok(LoadedModule {
            graph: desugared,
            contract,
        })
    }

    /// Compute the public names of a module. `ExportSet::All` → all binding
    /// names (including import-projected bindings). `ExportSet::Names(ns)` →
    /// ns (plus expanded reexports — see `expand_reexports`).
    ///
    /// Per ADR-0069, imports become bindings via projection, and with no
    /// export declarations ALL bindings are public (including import bindings).
    /// So a module `import insert from "..." \n sort = ...` with no exports
    /// has public names `["insert", "sort"]`.
    pub fn public_names(&self, path: &str) -> Result<Vec<String>, LoadError> {
        let loaded = self.load(path)?;
        let mut names = match &loaded.graph.export_set {
            ExportSet::All => loaded
                .graph
                .bindings
                .iter()
                .map(|(n, _)| n.clone())
                .collect::<Vec<String>>(),
            ExportSet::Names(ns) => ns.clone(),
        };

        // Expand reexports: for each `export * from "path"`, query that
        // module's public names and add them.
        for reexport_path in &loaded.graph.reexports {
            let remote_names = self.public_names(reexport_path)?;
            for n in remote_names {
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }

        Ok(names)
    }

    /// Expand `export * from "path"` markers left by the desugar pass.
    /// For each path in `desugared.reexports`, query `public_names(path)`,
    /// and for each name add a binding
    /// `(name, Kernel::Project { field: name, record: ModuleRef { path } })`
    /// and add the name to the export set. Returns a new `DesugaredProgram`
    /// with reexports cleared and the export set updated.
    ///
    /// If the export set was `All`, it stays `All` (all bindings public,
    /// including the new reexport bindings). If it was `Names`, extend with
    /// the new names.
    pub fn expand_reexports(
        &self,
        desugared: &DesugaredProgram,
    ) -> Result<DesugaredProgram, LoadError> {
        let mut bindings = desugared.bindings.clone();
        let mut export_set = desugared.export_set.clone();
        let mut added_names: Vec<String> = Vec::new();

        for reexport_path in &desugared.reexports {
            let remote_names = self.public_names(reexport_path)?;
            for name in remote_names {
                // Avoid duplicate bindings.
                if bindings.iter().any(|(n, _)| n == &name) {
                    continue;
                }
                bindings.push((
                    name.clone(),
                    Kernel::Project {
                        field: name.clone(),
                        record: Box::new(Kernel::ModuleRef {
                            path: reexport_path.clone(),
                        }),
                    },
                ));
                added_names.push(name);
            }
        }

        // Update export set.
        match &mut export_set {
            ExportSet::All => {
                // All bindings are public; the new reexport bindings are
                // included automatically. Keep All.
            }
            ExportSet::Names(ns) => {
                for n in added_names {
                    if !ns.contains(&n) {
                        ns.push(n);
                    }
                }
            }
        }

        Ok(DesugaredProgram {
            bindings,
            export_set,
            reexports: Vec::new(), // cleared
        })
    }
}

// === Section 7: Kernel traversal helpers ===

/// Does this kernel tree contain any `Kernel::ModuleRef` node?
pub fn contains_module_ref(k: &Kernel) -> bool {
    match k {
        Kernel::ModuleRef { .. } => true,
        Kernel::Seq { fst, snd } => contains_module_ref(fst) || contains_module_ref(snd),
        Kernel::First { inner } => contains_module_ref(inner),
        Kernel::Choice { l, r } => contains_module_ref(l) || contains_module_ref(r),
        Kernel::Loop { body } => contains_module_ref(body),
        Kernel::App { callee, args } => {
            contains_module_ref(callee) || args.iter().any(contains_module_ref)
        }
        Kernel::Arr { args, .. } => args.iter().any(contains_module_ref),
        Kernel::Project { record, .. } => contains_module_ref(record),
        Kernel::Var(_) | Kernel::Const(_) => false,
    }
}

/// Does this kernel tree contain any `Kernel::Project` node?
pub fn contains_project(k: &Kernel) -> bool {
    match k {
        Kernel::Project { record, .. } => true || contains_project(record),
        Kernel::Seq { fst, snd } => contains_project(fst) || contains_project(snd),
        Kernel::First { inner } => contains_project(inner),
        Kernel::Choice { l, r } => contains_project(l) || contains_project(r),
        Kernel::Loop { body } => contains_project(body),
        Kernel::App { callee, args } => {
            contains_project(callee) || args.iter().any(contains_project)
        }
        Kernel::Arr { args, .. } => args.iter().any(contains_project),
        Kernel::Var(_) | Kernel::Const(_) | Kernel::ModuleRef { .. } => false,
    }
}

/// Recursively transform a kernel, applying a substitution function to each
/// node bottom-up.
fn transform_kernel<F>(k: &Kernel, f: &F) -> Kernel
where
    F: Fn(&Kernel) -> Kernel,
{
    // First recurse into children, then apply f to the result.
    let transformed = match k {
        Kernel::Arr { op, args } => Kernel::Arr {
            op: op.clone(),
            args: args.iter().map(|a| transform_kernel(a, f)).collect(),
        },
        Kernel::Seq { fst, snd } => Kernel::Seq {
            fst: Box::new(transform_kernel(fst, f)),
            snd: Box::new(transform_kernel(snd, f)),
        },
        Kernel::First { inner } => Kernel::First {
            inner: Box::new(transform_kernel(inner, f)),
        },
        Kernel::Choice { l, r } => Kernel::Choice {
            l: Box::new(transform_kernel(l, f)),
            r: Box::new(transform_kernel(r, f)),
        },
        Kernel::Loop { body } => Kernel::Loop {
            body: Box::new(transform_kernel(body, f)),
        },
        Kernel::App { callee, args } => Kernel::App {
            callee: Box::new(transform_kernel(callee, f)),
            args: args.iter().map(|a| transform_kernel(a, f)).collect(),
        },
        Kernel::Project { field, record } => Kernel::Project {
            field: field.clone(),
            record: Box::new(transform_kernel(record, f)),
        },
        other => other.clone(),
    };
    f(&transformed)
}

/// Collect all module paths referenced by `Kernel::ModuleRef` nodes in the
/// tree (deduplicated, preserving first-seen order).
fn collect_module_refs(k: &Kernel) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    collect_module_refs_into(k, &mut paths);
    paths
}

fn collect_module_refs_into(k: &Kernel, out: &mut Vec<String>) {
    match k {
        Kernel::ModuleRef { path } => {
            if !out.contains(path) {
                out.push(path.clone());
            }
        }
        Kernel::Seq { fst, snd } => {
            collect_module_refs_into(fst, out);
            collect_module_refs_into(snd, out);
        }
        Kernel::First { inner } => collect_module_refs_into(inner, out),
        Kernel::Choice { l, r } => {
            collect_module_refs_into(l, out);
            collect_module_refs_into(r, out);
        }
        Kernel::Loop { body } => collect_module_refs_into(body, out),
        Kernel::App { callee, args } => {
            collect_module_refs_into(callee, out);
            for a in args {
                collect_module_refs_into(a, out);
            }
        }
        Kernel::Arr { args, .. } => {
            for a in args {
                collect_module_refs_into(a, out);
            }
        }
        Kernel::Project { record, .. } => collect_module_refs_into(record, out),
        Kernel::Var(_) | Kernel::Const(_) => {}
    }
}

/// Build a record literal kernel from a list of (name, kernel) pairs,
/// representing a module's public bindings. Each field is
/// `arr("field", [Const(Str(name)), kernel])`, wrapped in
/// `arr("record", [...])`, wrapped in `arr("module_record", [...])`.
fn build_module_record(public_bindings: &[(String, Kernel)]) -> Kernel {
    let fields: Vec<Kernel> = public_bindings
        .iter()
        .map(|(name, k)| {
            Kernel::arr(
                "field",
                vec![Kernel::Const(Lit::Str(name.clone())), k.clone()],
            )
        })
        .collect();
    let record = Kernel::arr("record", fields);
    Kernel::arr("module_record", vec![record])
}

// === Section 8: Strategy A — normalize_compile_time (eager, static IR) ===

/// Normalization Strategy A (ADR-0069 §5): compile-time desugar.
///
/// Resolves the entry module, parse+desugar it, then walks its bindings:
/// every `Kernel::ModuleRef { path }` is replaced by an inlined record literal
/// (`arr("module_record", [arr("record", [...])])`) containing the resolved
/// module's PUBLIC bindings. Recurses transitively with cycle detection.
///
/// The result is a self-contained `DesugaredProgram` where no `ModuleRef`
/// remains (all inlined as record literals). `Project` nodes remain — they
/// now project from an inlined record literal. This is correct for Strategy A.
pub fn normalize_compile_time(
    entry_path: &str,
    loader: &ModuleLoader,
) -> Result<DesugaredProgram, LoadError> {
    let mut visited: Vec<String> = Vec::new();
    normalize_compile_time_inner(entry_path, loader, &mut visited)
}

fn normalize_compile_time_inner(
    path: &str,
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
) -> Result<DesugaredProgram, LoadError> {
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

    // Load the module.
    let loaded = loader.load(path)?;
    let desugared = loader.expand_reexports(&loaded.graph)?;

    // Collect all module paths referenced in this module's bindings.
    let ref_paths: Vec<String> = desugared
        .bindings
        .iter()
        .flat_map(|(_, k)| collect_module_refs(k))
        .collect::<Vec<String>>();

    // Recursively normalize each referenced module and build a replacement map:
    // path -> inlined module record kernel.
    let mut inline_map: HashMap<String, Kernel> = HashMap::new();
    for ref_path in &ref_paths {
        if inline_map.contains_key(ref_path) {
            continue;
        }
        let child = normalize_compile_time_inner(ref_path, loader, visited)?;
        let pub_binds = public_bindings_from(&child);
        inline_map.insert(ref_path.clone(), build_module_record(&pub_binds));
    }

    // Replace every ModuleRef { path } in every binding with the inlined record.
    let new_bindings: Vec<(String, Kernel)> = desugared
        .bindings
        .iter()
        .map(|(name, k)| {
            let new_k = replace_module_refs(k, &inline_map);
            (name.clone(), new_k)
        })
        .collect();

    // Pop this path from the visited stack (allow diamond dependencies).
    visited.pop();

    Ok(DesugaredProgram {
        bindings: new_bindings,
        export_set: desugared.export_set,
        reexports: Vec::new(),
    })
}

/// Extract public bindings from a DesugaredProgram (assumes reexports already
/// expanded).
fn public_bindings_from(desugared: &DesugaredProgram) -> Vec<(String, Kernel)> {
    let names = match &desugared.export_set {
        ExportSet::All => desugared
            .bindings
            .iter()
            .map(|(n, _)| n.clone())
            .collect::<Vec<String>>(),
        ExportSet::Names(ns) => ns.clone(),
    };
    desugared
        .bindings
        .iter()
        .filter(|(n, _)| names.contains(n))
        .cloned()
        .collect()
}

/// Replace every `Kernel::ModuleRef { path }` with `inline_map[path]` (cloned).
/// Paths not in the map are left as-is (should not happen if all refs were
/// collected).
fn replace_module_refs(k: &Kernel, map: &HashMap<String, Kernel>) -> Kernel {
    transform_kernel(k, &|node| match node {
        Kernel::ModuleRef { path } => match map.get(path) {
            Some(replacement) => replacement.clone(),
            None => Kernel::ModuleRef {
                path: path.clone(),
            },
        },
        other => other.clone(),
    })
}

// === Section 9: Strategy B — flatten_program (whole-program flattening) ===

/// Normalization Strategy B (ADR-0069 §5): whole-program flattening.
///
/// Like Strategy A but goes further: after inlining all module records, also
/// resolve every `Kernel::Project { field, record }` to the actual field
/// binding's kernel (look up `field` in the inlined record), and every
/// `Kernel::App { callee, args }` where callee is a `Var(name)` that resolves
/// to an inlined binding — substitute the callee's kernel. The result is a
/// SINGLE kernel graph with no `ModuleRef`, no `Project`, and no top-level
/// `App` to imported names.
///
/// The entry's "main arrow" is: if the entry has a single binding, that
/// binding's (flattened) kernel; if multiple bindings, the record of all
/// PUBLIC bindings (flattened). Returns that single `Kernel`.
pub fn flatten_program(entry_path: &str, loader: &ModuleLoader) -> Result<Kernel, LoadError> {
    let mut visited: Vec<String> = Vec::new();
    flatten_program_inner(entry_path, loader, &mut visited)
}

fn flatten_program_inner(
    path: &str,
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
) -> Result<Kernel, LoadError> {
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

    // Load and expand reexports.
    let loaded = loader.load(path)?;
    let desugared = loader.expand_reexports(&loaded.graph)?;

    // Collect all module paths referenced.
    let ref_paths: Vec<String> = desugared
        .bindings
        .iter()
        .flat_map(|(_, k)| collect_module_refs(k))
        .collect::<Vec<String>>();

    // Recursively flatten each referenced module and build a map:
    // path -> (public bindings as (name, flattened kernel) pairs).
    let mut module_records: HashMap<String, Vec<(String, Kernel)>> = HashMap::new();
    for ref_path in &ref_paths {
        if module_records.contains_key(ref_path) {
            continue;
        }
        let _child_kernel = flatten_program_inner(ref_path, loader, visited)?;
        // The child kernel IS the module's main arrow. For a multi-binding
        // module, it's a record; for a single-binding module, it's that
        // binding's kernel. We need the public bindings as name->kernel pairs.
        // Re-load to get the bindings, then flatten each binding.
        let child_loaded = loader.load(ref_path)?;
        let child_expanded = loader.expand_reexports(&child_loaded.graph)?;
        let child_pub = public_bindings_from(&child_expanded);
        // Flatten each binding: replace ModuleRefs with inlined records, then
        // resolve Projects and App-to-Vars.
        let mut flattened_binds: Vec<(String, Kernel)> = Vec::new();
        for (bname, bk) in &child_pub {
            let inlined = inline_module_refs(bk, &module_records, ref_path, loader, visited)?;
            let resolved = resolve_projects_and_apps(&inlined, &child_pub);
            flattened_binds.push((bname.clone(), resolved));
        }
        module_records.insert(ref_path.clone(), flattened_binds);
    }

    // Now flatten this module's bindings.
    let pub_binds = public_bindings_from(&desugared);
    let mut flattened_bindings: Vec<(String, Kernel)> = Vec::new();
    for (name, k) in &pub_binds {
        let inlined = inline_module_refs(k, &module_records, path, loader, visited)?;
        let resolved = resolve_projects_and_apps(&inlined, &pub_binds);
        flattened_bindings.push((name.clone(), resolved));
    }

    visited.pop();

    // The entry's main arrow:
    if flattened_bindings.len() == 1 {
        Ok(flattened_bindings[0].1.clone())
    } else {
        // Build a record of all public bindings.
        let fields: Vec<Kernel> = flattened_bindings
            .iter()
            .map(|(name, k)| {
                Kernel::arr(
                    "field",
                    vec![Kernel::Const(Lit::Str(name.clone())), k.clone()],
                )
            })
            .collect();
        Ok(Kernel::arr("record", fields))
    }
}

/// Replace every `ModuleRef { path }` with an inlined record of the module's
/// public bindings (already flattened). Uses the `module_records` map; if a
/// path isn't in the map yet, recursively flatten it.
fn inline_module_refs(
    k: &Kernel,
    module_records: &HashMap<String, Vec<(String, Kernel)>>,
    _current_path: &str,
    loader: &ModuleLoader,
    visited: &mut Vec<String>,
) -> Result<Kernel, LoadError> {
    // Collect refs in this kernel.
    let refs = collect_module_refs(k);
    let mut inline_map: HashMap<String, Kernel> = HashMap::new();
    for ref_path in &refs {
        if inline_map.contains_key(ref_path) {
            continue;
        }
        if let Some(binds) = module_records.get(ref_path) {
            inline_map.insert(ref_path.clone(), build_module_record(binds));
        } else {
            // Need to flatten this child.
            let _child_kernel = flatten_program_inner(ref_path, loader, visited)?;
            // Re-load to get bindings and flatten them.
            let child_loaded = loader.load(ref_path)?;
            let child_expanded = loader.expand_reexports(&child_loaded.graph)?;
            let child_pub = public_bindings_from(&child_expanded);
            let mut flattened_binds: Vec<(String, Kernel)> = Vec::new();
            for (bname, bk) in &child_pub {
                let inlined = inline_module_refs(
                    bk,
                    module_records,
                    ref_path,
                    loader,
                    visited,
                )?;
                let resolved = resolve_projects_and_apps(&inlined, &child_pub);
                flattened_binds.push((bname.clone(), resolved));
            }
            inline_map.insert(ref_path.clone(), build_module_record(&flattened_binds));
        }
    }
    Ok(replace_module_refs(k, &inline_map))
}

/// Resolve `Project { field, record }` nodes: if `record` is an inlined
/// module record (`arr("module_record", [arr("record", [field_kernels])])`),
/// look up `field` in the field kernels and substitute. Also resolve
/// `App { callee: Var(name), args }` where `name` is a binding in `bindings`:
/// substitute the callee's kernel (keeping args as a Seq prefix).
///
/// This is the core of Strategy B: no Project, no ModuleRef, no App-to-import.
fn resolve_projects_and_apps(k: &Kernel, bindings: &[(String, Kernel)]) -> Kernel {
    transform_kernel(k, &|node| match node {
        Kernel::Project { field, record } => {
            // Try to resolve the project from an inlined record.
            if let Some(target) = lookup_field_in_record(record, field) {
                target
            } else {
                Kernel::Project {
                    field: field.clone(),
                    record: Box::new(record.as_ref().clone()),
                }
            }
        }
        Kernel::App { callee, args } => {
            // If callee is a Var(name) that matches a binding, substitute.
            if let Kernel::Var(name) = callee.as_ref() {
                if let Some((_, target_k)) = bindings.iter().find(|(n, _)| n == name) {
                    // App to a resolved arrow: Seq(args, target). For multiple
                    // args, Seq them together first.
                    let args_kernel = if args.len() == 1 {
                        args[0].clone()
                    } else if args.is_empty() {
                        Kernel::arr("unit", vec![])
                    } else {
                        Kernel::arr("tuple", args.clone())
                    };
                    return Kernel::seq(args_kernel, target_k.clone());
                }
            }
            Kernel::App {
                callee: callee.clone(),
                args: args.clone(),
            }
        }
        other => other.clone(),
    })
}

/// Look up a field by name in an inlined module record. The record is expected
/// to be `arr("module_record", [arr("record", [field_kernels])])` where each
/// field kernel is `arr("field", [Const(Str(name)), value])`. Returns the
/// value kernel if found.
fn lookup_field_in_record(record: &Kernel, field: &str) -> Option<Kernel> {
    // Unwrap module_record wrapper.
    let inner = match record {
        Kernel::Arr { op, args } if op == "module_record" && !args.is_empty() => &args[0],
        _ => return None,
    };
    // inner should be arr("record", [field_kernels])
    let fields = match inner {
        Kernel::Arr { op, args } if op == "record" => args,
        _ => return None,
    };
    for f in fields {
        if let Kernel::Arr { op, args } = f {
            if op == "field" && args.len() == 2 {
                if let Kernel::Const(Lit::Str(name)) = &args[0] {
                    if name == field {
                        return Some(args[1].clone());
                    }
                }
            }
        }
    }
    None
}

// === Section 10: CountingResolver (for cache-hit verification in tests) ===

/// A wrapper resolver that counts how many times `resolve` is called. Used in
/// tests to verify that `CachingResolver` and `ModuleLoader` caches prevent
/// redundant resolution. The count is shared via `Rc<Cell<usize>>` so it can
/// be read externally even after the resolver is boxed.
pub struct CountingResolver {
    inner: Box<dyn ModuleResolver>,
    count: Rc<Cell<usize>>,
}

impl CountingResolver {
    pub fn new(inner: Box<dyn ModuleResolver>) -> Self {
        CountingResolver {
            inner,
            count: Rc::new(Cell::new(0)),
        }
    }

    /// Create a CountingResolver and return a shared counter handle alongside
    /// it, so tests can read the call count after the resolver is boxed.
    pub fn new_with_counter(
        inner: Box<dyn ModuleResolver>,
    ) -> (Self, Rc<Cell<usize>>) {
        let count = Rc::new(Cell::new(0));
        let r = CountingResolver {
            inner,
            count: count.clone(),
        };
        (r, count)
    }

    pub fn call_count(&self) -> usize {
        self.count.get()
    }
}

impl ModuleResolver for CountingResolver {
    fn resolve(&self, path: &str) -> Result<ResolvedModule, ResolveError> {
        self.count.set(self.count.get() + 1);
        self.inner.resolve(path)
    }

    fn origin(&self) -> &'static str {
        self.inner.origin()
    }
}

// === Section 11: fn main() — test suite (PoC style) ===

fn check(label: &str, ok: bool) {
    if ok {
        println!("  PASS  {}", label);
    } else {
        println!("  FAIL  {}", label);
        std::process::exit(1);
    }
}

fn build_test_memory_resolver() -> MemoryResolver {
    let mut m = MemoryResolver::new();
    // collection/insert — a single binding (a loop arrow).
    m.insert(
        "collection/insert",
        "insert = rec { compare >>> (cons ||| cons) }",
    );
    // collection/sort — imports insert, defines sort.
    m.insert(
        "collection/sort",
        "import insert from \"collection/insert\"\nsort = rec { insert >>> id }",
    );
    // collection — multi-name file (record arrow).
    m.insert(
        "collection",
        "cons = compare\nhead = id\nsort = rec { compare >>> id }",
    );
    // main — imports sort, uses it.
    m.insert(
        "main",
        "import sort from \"collection/sort\"\nmain = sort([3, 1, 2])",
    );
    // reexport — export * from "collection".
    m.insert("reexport", "export * from \"collection\"");
    // cycle modules.
    m.insert("cycle_a", "import b from \"cycle_b\"\nx = b");
    m.insert("cycle_b", "import a from \"cycle_a\"\ny = a");
    m
}

fn main() {
    println!("handl v0.1 — Module System Tests (ADR-0069 §4–§5)");
    println!("==================================================\n");

    // --- Section 1: Resolvers ---
    println!("Section 1: Resolvers");
    {
        let mem = build_test_memory_resolver();

        // MemoryResolver: Ok with correct source.
        let r = mem.resolve("collection/sort");
        match r {
            Ok(m) => {
                check(
                    "MemoryResolver resolves collection/sort",
                    m.source.contains("import insert"),
                );
                check(
                    "MemoryResolver origin is 'memory'",
                    mem.origin() == "memory",
                );
            }
            Err(_) => check("MemoryResolver resolves collection/sort", false),
        }

        // MemoryResolver: NotFound for missing.
        match mem.resolve("missing") {
            Ok(_) => check("MemoryResolver NotFound for missing", false),
            Err(e) => check(
                "MemoryResolver NotFound for missing",
                matches!(e.kind, ResolveErrorKind::NotFound),
            ),
        }

        // ChainResolver: memory + FileResolver (temp dir).
        let mut chain_resolvers: Vec<Box<dyn ModuleResolver>> = Vec::new();
        let mut mem2 = MemoryResolver::new();
        mem2.insert("collection/sort", "sort = id");
        chain_resolvers.push(Box::new(mem2));
        chain_resolvers.push(Box::new(FileResolver::new(PathBuf::from("/tmp/handl_test_nonexistent"))));
        let chain = ChainResolver::new(chain_resolvers);

        match chain.resolve("collection/sort") {
            Ok(m) => check(
                "ChainResolver finds memory module",
                m.source == "sort = id",
            ),
            Err(_) => check("ChainResolver finds memory module", false),
        }
        match chain.resolve("totally/missing") {
            Ok(_) => check("ChainResolver NotFound for missing", false),
            Err(e) => check(
                "ChainResolver NotFound for missing",
                matches!(e.kind, ResolveErrorKind::NotFound),
            ),
        }
        check("ChainResolver origin is 'chain'", chain.origin() == "chain");

        // CachingResolver: returns Ok twice (cache hit on second).
        let mut mem3 = MemoryResolver::new();
        mem3.insert("collection/sort", "sort = id");
        let cache = CachingResolver::new(Box::new(mem3));
        let r1 = cache.resolve("collection/sort");
        let r2 = cache.resolve("collection/sort");
        check(
            "CachingResolver returns Ok twice",
            r1.is_ok() && r2.is_ok(),
        );
        check(
            "CachingResolver returns same content",
            r1.unwrap().source == r2.unwrap().source,
        );
    }

    // --- Section 2: ModuleLoader (Strategy C — two caches) ---
    println!("\nSection 2: ModuleLoader (Strategy C — two caches)");
    {
        let mem = build_test_memory_resolver();
        let (counting, counter) = CountingResolver::new_with_counter(Box::new(mem));
        let loader = ModuleLoader::new(Box::new(counting));

        // load("collection/sort") -> Ok; verify bindings contain "insert" as
        // a Project from ModuleRef.
        match loader.load("collection/sort") {
            Ok(loaded) => {
                let insert_binding = loaded
                    .graph
                    .bindings
                    .iter()
                    .find(|(n, _)| n == "insert");
                match insert_binding {
                    Some((_, k)) => {
                        check(
                            "load(collection/sort) has insert as Project from ModuleRef",
                            matches!(
                                k,
                                Kernel::Project { field, record }
                                    if field == "insert"
                                        && matches!(record.as_ref(), Kernel::ModuleRef { path } if path == "collection/insert")
                            ),
                        );
                    }
                    None => check("load(collection/sort) has insert binding", false),
                }
            }
            Err(e) => {
                check("load(collection/sort) succeeds", false);
                eprintln!("  error: {}", e);
            }
        }

        // Cache hit: loading the same path twice calls resolve only once.
        // We already loaded "collection/sort" once above. Load again.
        let count_before = counter.get();
        let _ = loader.load("collection/sort");
        let count_after = counter.get();
        check(
            "cache hit: second load does not call resolve",
            count_before == count_after,
        );

        // public_names("collection") -> ["cons", "head", "sort"] (All).
        match loader.public_names("collection") {
            Ok(names) => {
                check(
                    "public_names(collection) = [cons, head, sort]",
                    names.len() == 3
                        && names.contains(&"cons".to_string())
                        && names.contains(&"head".to_string())
                        && names.contains(&"sort".to_string()),
                );
            }
            Err(_) => check("public_names(collection) succeeds", false),
        }

        // public_names("collection/sort") -> ["insert", "sort"] (All, includes
        // the import-projected binding "insert").
        match loader.public_names("collection/sort") {
            Ok(names) => {
                check(
                    "public_names(collection/sort) = [insert, sort]",
                    names.len() == 2
                        && names.contains(&"insert".to_string())
                        && names.contains(&"sort".to_string()),
                );
            }
            Err(_) => check("public_names(collection/sort) succeeds", false),
        }

        // public_names("main") -> ["sort", "main"] (All, includes import).
        match loader.public_names("main") {
            Ok(names) => {
                check(
                    "public_names(main) = [sort, main]",
                    names.len() == 2
                        && names.contains(&"sort".to_string())
                        && names.contains(&"main".to_string()),
                );
            }
            Err(_) => check("public_names(main) succeeds", false),
        }
    }

    // --- Section 3: expand_reexports ---
    println!("\nSection 3: expand_reexports");
    {
        let mem = build_test_memory_resolver();
        let loader = ModuleLoader::new(Box::new(mem));

        match loader.load("reexport") {
            Ok(loaded) => {
                // Before expansion: export_set = Names([]), reexports = ["collection"].
                check(
                    "reexport before expansion: Names([])",
                    matches!(&loaded.graph.export_set, ExportSet::Names(ns) if ns.is_empty()),
                );
                check(
                    "reexport before expansion: reexports = [collection]",
                    loaded.graph.reexports == vec!["collection".to_string()],
                );

                match loader.expand_reexports(&loaded.graph) {
                    Ok(expanded) => {
                        // Should have bindings for cons, head, sort (each a
                        // Project from "collection").
                        let has_cons = expanded.bindings.iter().any(|(n, k)| {
                            n == "cons"
                                && matches!(
                                    k,
                                    Kernel::Project { field, record }
                                        if field == "cons"
                                            && matches!(record.as_ref(), Kernel::ModuleRef { path } if path == "collection")
                                )
                        });
                        let has_head = expanded.bindings.iter().any(|(n, k)| {
                            n == "head"
                                && matches!(
                                    k,
                                    Kernel::Project { field, record }
                                        if field == "head"
                                            && matches!(record.as_ref(), Kernel::ModuleRef { path } if path == "collection")
                                )
                        });
                        let has_sort = expanded.bindings.iter().any(|(n, k)| {
                            n == "sort"
                                && matches!(
                                    k,
                                    Kernel::Project { field, record }
                                        if field == "sort"
                                            && matches!(record.as_ref(), Kernel::ModuleRef { path } if path == "collection")
                                )
                        });
                        check("expand_reexports adds cons binding", has_cons);
                        check("expand_reexports adds head binding", has_head);
                        check("expand_reexports adds sort binding", has_sort);

                        // export_set should be Names(["cons","head","sort"]).
                        match &expanded.export_set {
                            ExportSet::Names(ns) => {
                                check(
                                    "expand_reexports export_set = Names([cons,head,sort])",
                                    ns.len() == 3
                                        && ns.contains(&"cons".to_string())
                                        && ns.contains(&"head".to_string())
                                        && ns.contains(&"sort".to_string()),
                                );
                            }
                            _ => check("expand_reexports export_set is Names", false),
                        }
                        // reexports cleared.
                        check(
                            "expand_reexports clears reexports",
                            expanded.reexports.is_empty(),
                        );
                    }
                    Err(e) => {
                        check("expand_reexports succeeds", false);
                        eprintln!("  error: {}", e);
                    }
                }
            }
            Err(e) => {
                check("load(reexport) succeeds", false);
                eprintln!("  error: {}", e);
            }
        }
    }

    // --- Section 4: Strategy A — normalize_compile_time ---
    println!("\nSection 4: Strategy A — normalize_compile_time");
    {
        let mem = build_test_memory_resolver();
        let loader = ModuleLoader::new(Box::new(mem));

        match normalize_compile_time("main", &loader) {
            Ok(normalized) => {
                // No ModuleRef anywhere.
                let any_ref = normalized
                    .bindings
                    .iter()
                    .any(|(_, k)| contains_module_ref(k));
                check("Strategy A: no ModuleRef remains", !any_ref);

                // Project nodes DO remain (Strategy A keeps projections).
                let any_project = normalized
                    .bindings
                    .iter()
                    .any(|(_, k)| contains_project(k));
                check("Strategy A: Project nodes remain", any_project);

                // The "sort" binding's Project now has a record that is an
                // inlined arr("module_record", ...) rather than a ModuleRef.
                let sort_binding = normalized.bindings.iter().find(|(n, _)| n == "sort");
                match sort_binding {
                    Some((_, k)) => {
                        let is_inlined = match k {
                            Kernel::Project { record, .. } => {
                                matches!(
                                    record.as_ref(),
                                    Kernel::Arr { op, .. } if op == "module_record"
                                )
                            }
                            _ => false,
                        };
                        check(
                            "Strategy A: sort binding Project has inlined module_record",
                            is_inlined,
                        );
                    }
                    None => check("Strategy A: sort binding exists", false),
                }

                // Print the normalized bindings for visibility.
                println!("  Normalized main bindings:");
                for (name, k) in &normalized.bindings {
                    println!("    {} = {:?}", name, k);
                }
            }
            Err(e) => {
                check("normalize_compile_time(main) succeeds", false);
                eprintln!("  error: {}", e);
            }
        }
    }

    // --- Section 5: Strategy B — flatten_program ---
    println!("\nSection 5: Strategy B — flatten_program");
    {
        let mem = build_test_memory_resolver();
        let loader = ModuleLoader::new(Box::new(mem));

        match flatten_program("main", &loader) {
            Ok(flat) => {
                check("Strategy B: no ModuleRef", !contains_module_ref(&flat));
                check("Strategy B: no Project", !contains_project(&flat));

                // Print the flattened kernel.
                println!("  Flattened main kernel:");
                println!("    {:?}", flat);
            }
            Err(e) => {
                check("flatten_program(main) succeeds", false);
                eprintln!("  error: {}", e);
            }
        }
    }

    // --- Section 6: Cycle detection ---
    println!("\nSection 6: Cycle detection");
    {
        let mem = build_test_memory_resolver();
        let loader = ModuleLoader::new(Box::new(mem));

        match normalize_compile_time("cycle_a", &loader) {
            Ok(_) => check("cycle_a normalization returns Cycle error", false),
            Err(e) => {
                check(
                    "cycle_a normalization returns Cycle error",
                    matches!(e, LoadError::Cycle(_)),
                );
                println!("  Cycle error: {}", e);
            }
        }

        // Also test cycle detection in flatten_program.
        let mem2 = build_test_memory_resolver();
        let loader2 = ModuleLoader::new(Box::new(mem2));
        match flatten_program("cycle_a", &loader2) {
            Ok(_) => check("cycle_a flatten returns Cycle error", false),
            Err(e) => {
                check(
                    "cycle_a flatten returns Cycle error",
                    matches!(e, LoadError::Cycle(_)),
                );
            }
        }
    }

    // --- Section 7: Capability boundary (ADR-0046) ---
    println!("\nSection 7: Capability boundary (ADR-0046)");
    {
        // A ChainResolver with ONLY a MemoryResolver (no file resolver).
        // Paths in memory succeed; paths needing a file resolver return
        // NotFound. This models a host denying the file capability.
        let mut mem = MemoryResolver::new();
        mem.insert("allowed", "x = id");
        let chain = ChainResolver::new(vec![Box::new(mem)]);

        match chain.resolve("allowed") {
            Ok(_) => check("capability: allowed path resolves", true),
            Err(_) => check("capability: allowed path resolves", false),
        }
        match chain.resolve("needs/file") {
            Ok(_) => check("capability: denied path returns NotFound", false),
            Err(e) => check(
                "capability: denied path returns NotFound",
                matches!(e.kind, ResolveErrorKind::NotFound),
            ),
        }
        println!("  Note: resolver set = [MemoryResolver only]; file capability denied.");
    }

    println!("\nAll module-system tests passed.");
}
