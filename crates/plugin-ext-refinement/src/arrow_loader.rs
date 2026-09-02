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
//! and inlining its `Scheme` graph — matching the hand-built graphs in
//! `e2e_tests.rs` where `sort` inlines `insert`'s full graph.

use std::collections::HashMap;
use std::fs;

use handl_language::Scheme;

use crate::arrow_parser::{parse_arrow_file, ParsedFile};
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::RefinementResult;

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

/// Load and infer: parse `.hnl` → `Scheme` → resolve references → `infer_refinement`.
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
/// name → `Scheme` graph map.
///
/// The key is the file stem (e.g. `insert.hnl` → `"insert"`).
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
            // Parse — skip files that fail (the parser is minimal)
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
