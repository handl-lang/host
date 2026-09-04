//! `textDocument/moniker` support for the handl LSP server.
//!
//! Returns stable, workspace-cross-cutting identifiers for the symbol under
//! the cursor. Monikers let language clients feed symbols into global
//! reference indexes (e.g. LSIF / code navigation) without relying on
//! ephemeral document URIs.
//!
//! Identifier schemes:
//!
//! * **Axioms / contracts** — a symbol defined inside a `contracts/` file
//!   uses `contracts/<filename>/<axiom_name>`.
//! * **Arrow files** — the file path relative to the kit root
//!   (`handl-packages/<pkg>/`) is used, with path separators normalised to
//!   `/` and the `.hnl` extension stripped. For example
//!   `/repo/handl-packages/kit-stdlib/collection/sort.hnl` yields the
//!   moniker `collection/sort`.
//! * **Local symbols** — identifiers that are not part of an arrow or
//!   contract file fall back to `<filename>/<name>`.
//!
//! Each moniker is a JSON object:
//!
//! ```json
//! { "scheme": "handl", "identifier": "collection/sort", "unique": "identifier" }
//! ```

// These entry points are not yet wired into the LSP dispatch loop (handlers.rs
// is intentionally untouched); suppress dead-code warnings until they are.
#![allow(dead_code)]

use serde_json::{Value, json};

use crate::refinement::find_arrows_root;
use crate::symbols::identifier_at_offset;

/// Compute `textDocument/moniker` results for the byte `offset` in `source`.
///
/// `file_path` is the filesystem path of the document (not a URI), used to
/// derive a stable cross-workspace identifier. Returns an empty vector when
/// the cursor is not on an identifier or no stable identifier can be derived.
pub fn moniker(source: &str, offset: usize, file_path: Option<&str>) -> Vec<Value> {
    // Resolve the identifier under the cursor.
    let (name, _) = match identifier_at_offset(source, offset) {
        Some(id) => id,
        None => return Vec::new(),
    };

    let identifier = match file_path {
        Some(path) => stable_identifier(path, &name),
        None => name.clone(),
    };

    vec![json!({
        "scheme": "handl",
        "identifier": identifier,
        "unique": "identifier",
    })]
}

/// Derive a stable moniker identifier for a symbol `name` defined in `path`.
///
/// * If `path` lives under a `contracts/` directory, the identifier is
///   `contracts/<filename>/<name>`.
/// * If `path` lives under a kit root (`handl-packages/<pkg>/`), the relative
///   path (minus the `.hnl` extension) is used as the identifier, ignoring
///   the symbol name — the file *is* the arrow.
/// * Otherwise the identifier is `<filename_stem>/<name>`.
fn stable_identifier(path: &str, name: &str) -> String {
    // Normalise backslashes (Windows) to forward slashes.
    let normalised = path.replace('\\', "/");

    // Contract files: contracts/<filename>/<axiom_name>.
    // Checked BEFORE the kit root, since contracts/ is a child of the kit
    // root and would otherwise be matched by find_arrows_root.
    if normalised.contains("/contracts/") {
        let filename = normalised
            .rsplit('/')
            .next()
            .unwrap_or(&normalised);
        let stem = filename.strip_suffix(".hnl").unwrap_or(filename);
        return format!("contracts/{}/{}", stem, name);
    }

    // Arrow files: identifier is the path relative to the kit root.
    if let Some(root) = find_arrows_root(&normalised) {
        let rel = normalised
            .strip_prefix(&root)
            .unwrap_or(&normalised);
        let rel = rel.strip_suffix(".hnl").unwrap_or(rel);
        return rel.to_string();
    }

    // Fallback: <filename_stem>/<name>.
    let filename = normalised.rsplit('/').next().unwrap_or(&normalised);
    let stem = filename.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(filename);
    format!("{}/{}", stem, name)
}
