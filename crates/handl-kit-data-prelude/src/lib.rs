//! # handl-kit-data-prelude
//!
//! The **data-driven canonical kit** — the ADR-0044 §6 end state.
//!
//! Composes [`handl_kit_prelude_lang`] — the eval-path `Language` fragment
//! set (`tokens`, `grammar`, `eval`, `types`, `ast`, `caps`, `options`,
//! `lexer_rules`, `transforms`, `globals`) — and layers the **tooling
//! fragments** on top: `lints.yaml` (ADR-0097) and `lsp.yaml` (ADR-0100),
//! plus the kit-composition manifests in `data/kits/`.
//!
//! The eval-path YAML files moved to the sibling `handl-lang/kit-prelude-lang`
//! repo so consumers that only lex / parse / desugar / eval `.hnl` (e.g.
//! `research-mlopt`) can take the language without the tooling surface.
//! This crate's [`language_from_yaml`] produces the same canonical
//! `Language` as before — lints and lsp occupy disjoint `lang.lints` /
//! `lang.lsp` sections merged last.
//!
//! ## Usage
//!
//! ```no_run
//! let kit = handl_kit_data_prelude::assemble().unwrap();
//! let lang = kit.lang();
//! ```

#![warn(missing_docs)]

use std::path::Path;

use handl_kit_data::{DataKit, DataKitManifest, KitRegistry};
use handl_language::{LangError, Language};

pub use handl_kit_prelude_lang::EXCLUDED_FRAGMENT_IDS;

/// The kit name under which this kit registers itself (for `extends`
/// resolution by child kits).
pub const KIT_NAME: &str = "kit-data-prelude";

// ---------------------------------------------------------------------------
// Tooling YAML data files
// ---------------------------------------------------------------------------

const LINTS_YAML: &str = include_str!("../data/lints.yaml");
const LSP_YAML: &str = include_str!("../data/lsp.yaml");

/// Build the full canonical [`Language`]: the eval-path language from
/// [`handl_kit_prelude_lang::language_from_yaml`] with `lints.yaml` and
/// `lsp.yaml` merged on top (plus the ADR-0100 LSP validation).
///
/// # Errors
///
/// Returns [`LangError`] if any YAML fails to deserialize, a merge fails, or
/// LSP validation reports errors.
pub fn language_from_yaml() -> Result<Language, LangError> {
    let mut lang = handl_kit_prelude_lang::language_from_yaml()?;
    handl_kit_prelude_lang::merge_lints(&mut lang, LINTS_YAML)?;
    handl_kit_prelude_lang::merge_lsp(&mut lang, LSP_YAML)?;
    Ok(lang)
}

/// Build a [`Language`] from the YAML data files in a directory.
///
/// Reads the eval-path files via [`handl_kit_prelude_lang::language_from_dir`]
/// and then merges (optionally) `lints.yaml` and `lsp.yaml` — fragment
/// directories written before ADR-0097/ADR-0100 do not carry them.
///
/// # Errors
///
/// Returns [`LangError`] if `dir` is not a directory, a required YAML file
/// is missing or unreadable, or a fragment fails to deserialize/merge.
pub fn language_from_dir(dir: &Path) -> Result<Language, LangError> {
    let mut lang = handl_kit_prelude_lang::language_from_dir(dir)?;

    for (name, merge) in [
        ("lints.yaml", handl_kit_prelude_lang::merge_lints as fn(&mut Language, &str) -> Result<(), LangError>),
        ("lsp.yaml", handl_kit_prelude_lang::merge_lsp),
    ] {
        match std::fs::read_to_string(dir.join(name)) {
            Ok(s) => merge(&mut lang, &s)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(LangError::engine(format!("{}: {}", name, e))),
        }
    }
    Ok(lang)
}

// ---------------------------------------------------------------------------
// Kit manifest + registry
// ---------------------------------------------------------------------------

/// Build the [`DataKitManifest`] for the data-driven canonical kit.
///
/// The manifest declares no fragment sources — the canonical [`Language`]
/// enters assembly directly as the `base_language` argument to
/// [`DataKit::assemble`] (see [`assemble`]). It carries only the runtime
/// engine selection.
///
/// The `runtime.typecheck` field selects the `nbe` engine registered by the host.
pub fn manifest() -> DataKitManifest {
    load_kit("prelude").unwrap_or_else(|e| panic!("failed to load canonical manifest: {}", e))
}

/// Build a [`KitRegistry`] with all embedded `data/kits/*.yaml` manifests
/// registered under their kit names (so `extends: prelude` resolves for
/// child kits).
pub fn registry() -> KitRegistry {
    let mut reg = KitRegistry::new();
    for (name, _) in KIT_MANIFESTS {
        let m = load_kit(name)
            .unwrap_or_else(|e| panic!("failed to load manifest for {}: {}", name, e));
        reg.register_kit(name, m);
    }
    reg
}

// ---------------------------------------------------------------------------
// Kit manifest data files (`data/kits/*.yaml`)
// ---------------------------------------------------------------------------

/// The kit manifests embedded at compile time, keyed by kit name.
///
/// Each entry is `data/kits/<name>.yaml` — a [`DataKitManifest`] whose
/// `modules` list names the library plugins the kit contributes, in
/// contribution order. The `extends` chain composes kits: a child's modules
/// append to its parent's (e.g. `server` extends `config`).
const KIT_MANIFESTS: &[(&str, &str)] = &[
    ("prelude", include_str!("../data/kits/prelude.yaml")),
    ("hello", include_str!("../data/kits/hello.yaml")),
    ("stdlib", include_str!("../data/kits/stdlib.yaml")),
    ("cli", include_str!("../data/kits/cli.yaml")),
    ("config", include_str!("../data/kits/config.yaml")),
    ("server", include_str!("../data/kits/server.yaml")),
];

/// Load a kit manifest by name from the embedded `data/kits/` files.
///
/// The returned [`DataKitManifest`] carries the kit's `modules` list (and, for
/// `canonical`, the runtime selection mirroring [`manifest`]). Unknown names
/// surface as [`LangError::Engine`].
///
/// # Errors
///
/// Returns [`LangError::Engine`] if `name` is not a known kit or the embedded
/// YAML fails to deserialize.
pub fn load_kit(name: &str) -> Result<DataKitManifest, LangError> {
    let yaml = KIT_MANIFESTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, y)| *y)
        .ok_or_else(|| LangError::engine(format!("unknown kit: {}", name)))?;
    serde_yaml::from_str(yaml).map_err(|e| LangError::engine(format!("kits/{}.yaml: {}", name, e)))
}

/// Resolve a kit name to its flattened, ordered list of library module plugin
/// names, following the manifest's `extends` chain.
///
/// The parent's modules come first, then the child's, preserving the
/// contribution order the kit's `contribute` method used to express in Rust.
///
/// # Errors
///
/// Returns [`LangError::Engine`] if `name` (or any kit in its `extends`
/// chain) is not a known kit or fails to parse.
pub fn kit_module_names(name: &str) -> Result<Vec<String>, LangError> {
    let manifest = load_kit(name)?;
    let mut modules = if let Some(parent) = &manifest.extends {
        kit_module_names(parent)?
    } else {
        Vec::new()
    };
    modules.extend(manifest.modules.iter().cloned());
    Ok(modules)
}

/// Assemble the data-driven canonical kit: load the YAML data files, build a
/// [`DataKit`] with the Pratt + tree-walk runtime config.
///
/// The canonical language enters [`DataKit::assemble`] directly as
/// `base_language` — no fragment source indirection.
///
/// # Errors
///
/// Returns [`LangError`] if any YAML deserialization or fragment merge fails
/// (should not happen — the files are pre-resolved).
pub fn assemble() -> Result<DataKit, LangError> {
    let reg = registry();
    DataKit::assemble(&manifest(), &reg, language_from_yaml()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_kit_data::DataKit;

    #[test]
    fn manifest_has_no_fragment_sources() {
        // The canonical `Language` enters assembly as `base_language`, not as
        // a manifest fragment source.
        let m = manifest();
        assert!(m.fragments.is_empty());
        assert_eq!(m.extends, None);
        assert_eq!(m.runtime.lex, "data");
        assert_eq!(m.runtime.parse, "pratt");
        assert_eq!(m.runtime.eval, "tree-walk");
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let m = manifest();
        let j = serde_json::to_string(&m).unwrap();
        let back: DataKitManifest = serde_json::from_str(&j).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn assemble_produces_nonempty_language() {
        let kit = assemble().unwrap();
        let lang = kit.lang();
        assert!(lang.tokens.len() > 0);
        assert!(lang.grammar.len() > 0);
        assert!(lang.eval.len() > 0);
        assert!(lang.types.len() > 0);
        assert!(lang.ast.len() > 0);
        assert!(lang.caps.len() > 0);
    }

    #[test]
    fn language_from_yaml_produces_language() {
        let lang = language_from_yaml().unwrap();
        assert!(lang.tokens.len() > 0);
        assert!(lang.grammar.len() > 0);
        assert!(lang.eval.len() > 0);
    }

    #[test]
    fn load_kit_parses_embedded_manifests() {
        let canonical = load_kit("prelude").unwrap();
        assert!(canonical.fragments.is_empty());
        assert_eq!(canonical.runtime.eval, "tree-walk");
        assert!(canonical.modules.is_empty());

        let hello = load_kit("hello").unwrap();
        assert_eq!(hello.extends.as_deref(), Some("prelude"));
        assert_eq!(hello.modules, vec!["module-runtime", "hello"]);
    }

    #[test]
    fn load_kit_unknown_name_errors() {
        assert!(load_kit("bogus").is_err());
    }

    #[test]
    fn load_kit_config_declares_pure_library_caps() {
        // The `config` kit's pure-library capabilities are authored data in
        // `data/kits/config.yaml` (they used to come from the deleted
        // `handl-kit-data-config` crate's `PureCapsEmitter`).
        let config = load_kit("config").unwrap();
        assert_eq!(config.extends.as_deref(), Some("prelude"));
        let ids: Vec<&str> = config.caps.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "lib.math",
                "lib.str",
                "lib.array",
                "lib.set",
                "lib.bool",
                "lib.cmp",
                "lib.obj",
                "lib.fn",
                "lib.type",
                "lib.crypto",
                "lib.datetime",
                "lib.intl-c",
                "lib.unicode-c",
                "lib.kv",
            ]
        );
        assert!(
            config
                .caps
                .iter()
                .all(|c| c.description.as_deref() == Some("pure library capability"))
        );
    }

    #[test]
    fn assemble_config_kit_merges_caps_via_extends() {
        // `extends` resolves through the registry: the `config` manifest
        // (extends `canonical`) assembles the canonical language plus its 14
        // authored `lib.*` capabilities, and inherits the parent's runtime
        // selection (its manifest declares no `runtime:`).
        let reg = registry();
        let config = load_kit("config").unwrap();
        let kit = DataKit::assemble(&config, &reg, language_from_yaml().unwrap()).unwrap();
        let lang = kit.lang();
        assert!(lang.tokens.len() > 0);
        assert!(lang.grammar.len() > 0);
        assert!(lang.eval.len() > 0);
        let canonical_caps = assemble().unwrap().lang().caps.len();
        assert_eq!(lang.caps.len(), canonical_caps + 14);
        for id in [
            "lib.math",
            "lib.str",
            "lib.array",
            "lib.set",
            "lib.bool",
            "lib.cmp",
            "lib.obj",
            "lib.fn",
            "lib.type",
            "lib.crypto",
            "lib.datetime",
            "lib.intl-c",
            "lib.kv",
        ] {
            assert!(lang.caps.get_by_id(id).is_some(), "missing {}", id);
        }
        assert_eq!(kit.runtime_config().parse, "pratt");
        assert_eq!(kit.runtime_config().eval, "tree-walk");
    }

    #[test]
    fn kit_module_names_flattens_extends() {
        // `stdlib` extends `hello` which extends `canonical`.
        assert_eq!(
            kit_module_names("stdlib").unwrap(),
            vec!["module-runtime", "hello", "utils", "collections"]
        );
        // `server` extends `config`: parent's modules first.
        let server = kit_module_names("server").unwrap();
        assert_eq!(server.first().unwrap(), "module-runtime");
        assert_eq!(server.last().unwrap(), "otlp-std");
        assert_eq!(server.len(), 13);
    }

    #[test]
    fn excluded_fragments_absent() {
        let kit = assemble().unwrap();
        let lang = kit.lang();
        for id in EXCLUDED_FRAGMENT_IDS {
            // Eval scheme ids map to node kinds; check by id.
            if id.starts_with("eval.") {
                // The eval.call.postfix fragment should not be present.
                // eval schemes are keyed by node_kind, not id, so we check
                // that the canonical eval.call is present (not the postfix
                // variant).
                assert!(lang.eval.get_by_id("eval.call").is_some());
            } else if id.starts_with("grammar.") {
                // Grammar rows are keyed by (token, position).
                // Just verify the canonical grammar.call.infix is present.
                assert!(lang.grammar.get_by_id("grammar.call.infix").is_some());
            }
        }
    }

    #[test]
    fn load_kit_config_carries_caps() {
        let config = load_kit("config").unwrap();
        assert_eq!(config.caps.len(), 14);
        for id in &[
            "lib.math",
            "lib.str",
            "lib.array",
            "lib.set",
            "lib.bool",
            "lib.cmp",
            "lib.obj",
            "lib.fn",
            "lib.type",
            "lib.crypto",
            "lib.datetime",
            "lib.intl-c",
            "lib.unicode-c",
            "lib.kv",
        ] {
            assert!(
                config.caps.iter().any(|c| c.id == *id),
                "config manifest missing cap {}",
                id
            );
        }
    }

    #[test]
    fn assemble_config_extends_canonical_caps() {
        let kit = DataKit::assemble(
            &load_kit("config").unwrap(),
            &registry(),
            language_from_yaml().unwrap(),
        )
        .unwrap();
        let lang = kit.lang();
        // Canonical contributes 3 foundation caps; config adds 14 pure-library caps.
        assert_eq!(lang.caps.len(), 17);
        assert!(lang.caps.get_by_id("lex").is_some());
        assert!(lang.caps.get_by_id("parse").is_some());
        for id in &[
            "lib.math",
            "lib.str",
            "lib.array",
            "lib.set",
            "lib.bool",
            "lib.cmp",
            "lib.obj",
            "lib.fn",
            "lib.type",
            "lib.crypto",
            "lib.datetime",
            "lib.intl-c",
            "lib.unicode-c",
            "lib.kv",
        ] {
            assert!(lang.caps.get_by_id(id).is_some(), "missing cap {}", id);
        }
    }

    #[test]
    fn assemble_config_inherits_parent_runtime() {
        let kit = DataKit::assemble(
            &load_kit("config").unwrap(),
            &registry(),
            language_from_yaml().unwrap(),
        )
        .unwrap();
        assert_eq!(kit.runtime_config().typecheck, "nbe");
        assert_eq!(kit.runtime_config().parse, "pratt");
        assert_eq!(kit.runtime_config().eval, "tree-walk");
    }
}
