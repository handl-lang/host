//! Shared helpers for the `*-c` backend smoke tests.
//!
//! `lib-*-c` packages declare their cross-package imports as *package
//! deps* (`pkg.hnl` `deps:` — e.g. `lib-unicode-c` imports
//! `intl-c/handl-packages/generated/...`, `lib-noise-c` imports `rand-c`).
//! `Kit::new` wires an empty dependency map, so those specs resolve as
//! host-module names and the imports silently degrade. [`kit_with_deps`]
//! rebuilds the canonical kit with the sibling `lib-*` checkouts
//! registered under their package names — the same map `hnl`'s
//! `[dependencies]`/`pkg.hnl` resolution produces.

use std::collections::HashMap;
use std::path::PathBuf;

use handl_kit_prelude::{Kit, prelude_engine_registry};

/// Repo root relative to this crate (`host/crates/kit-prelude`).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

/// The canonical kit plus the `lib-*` package deps the `*-c` backends
/// declare in their `pkg.hnl` files:
///
/// - `"intl-c"` → `lib-intl-c` — the generated `icu-*` adapter/port
///   tree (`intl-c/handl-packages/generated/...` specs).
/// - `"rand-c"` → `lib-rand-c/src` — the package entry is
///   `src/index.hnl`, so the dep dir points at `src` (the loader's
///   `index.hnl` rule resolves the bare `rand-c` spec to it).
///
/// Dep entries that do not exist on disk are skipped so the helper
/// still builds a working kit on a partial checkout.
pub fn kit_with_deps() -> Kit {
    let assembled = handl_kit_data_prelude::assemble().expect("canonical data kit assembles");
    let lang = assembled.lang().clone();
    let config = assembled.runtime_config().clone();

    let mut deps = HashMap::new();
    for (name, rel) in [
        ("intl-c", "lib-intl-c"),
        ("rand-c", "lib-rand-c/src"),
    ] {
        let dir = repo_root().join(rel);
        if dir.is_dir() {
            deps.insert(name.to_string(), dir);
        }
    }

    Kit::build_kit(
        lang,
        config,
        None,
        deps,
        None,
        prelude_engine_registry(),
    )
    .expect("kit with lib deps must build")
}
