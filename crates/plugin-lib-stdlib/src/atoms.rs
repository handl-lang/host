//! # `atoms.yaml` — spec-driven op registries (P-013 T-149)
//!
//! The atom IDL replaces hand-wired `*_op_registry()` builders and
//! `*_OPS` name lists: a lib ships an `atoms.yaml` spec declaring each
//! atom's `{name, sig, impl}` and the loader builds [`OpRegistry`]
//! entries `{name → {fn, sig}}` from it — zero handwritten wiring.
//!
//! ## Spec format
//!
//! One file per lib (`atoms.yaml` at the package root, vendored by
//! `handl-pkg-vendor` like `pkg.hnl`). Two shapes:
//!
//! ```yaml
//! # single-module lib (lib-proc):
//! module: proc
//! atoms:
//!   - name: proc.argv
//!     sig: "Unit ~> (List String)"
//!     impl: proc_argv
//!     total: true
//! ```
//!
//! ```yaml
//! # multi-module lib (lib-utils):
//! modules:
//!   math:
//!     atoms:
//!       - { name: math.abs, sig: "Float ~> Float", impl: math_abs, total: true }
//!   str:
//!     atoms: [...]
//! ```
//!
//! Fields per atom:
//!
//! - `name` — the qualified dispatch key (`mod.fn`); also the record
//!   field name is its last `.`-segment.
//! - `sig` — handl signature text (`~>` canonical), interpreted by
//!   [`sig_to_type_value`] into the tagged checker type value stored on
//!   the registry entry (T-148). Optional: absent = unsigned atom.
//! - `impl` — key into the module's impl table: a `fn() ->
//!   Vec<(&'static str, Arc<dyn Function + Send + Sync>)>` in the
//!   matching `ops/*.rs` (e.g. [`crate::ops::process::proc_impls`]).
//! - `total` — metadata replacing the dead `guarantee:`/`requires:`/
//!   `termination:` decl clauses; parsed and carried, not yet consumed
//!   (capability gating is a later task).
//!
//! ## Registration mechanism
//!
//! No `inventory`/`linkme`/`ctor` dependency exists anywhere in the
//! workspace (verified: zero Cargo.toml/lock hits), so impls do **not**
//! self-register at link time. Instead each migrated `ops/*.rs` exposes
//! one `*_impls()` fn returning its `(impl_key → Function)` table — the
//! "impl table" the proposal sanctions as the only irreducibly
//! hand-written part (it *is* the computation). The loader merges spec
//! + table; adding an atom is one yaml line + one impl fn + one table
//! row. If `inventory` is ever adopted workspace-wide this module is
//! the single place to swap the collection strategy.

use std::collections::BTreeMap;
use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{OpRegistry, sig_to_type_value};
use serde::Deserialize;

/// One `impl:` table entry: spec key → the `Function` impl (the handl).
pub type AtomImpl = (&'static str, Arc<dyn Function + Send + Sync>);

/// A module's impl table — what `ops::<mod>::*_impls()` returns.
pub type ImplTable = fn() -> Vec<AtomImpl>;

/// One kernel-side `impl:` table entry: spec key → the kernel
/// [`handl_ops::Op`] impl. Used by [`registry_from_atoms_kernel`] for
/// modules whose atoms live in `handl-lib-*`/`lib-*` crates (which speak
/// the kernel op ABI, not the host `Function` trait).
pub type KernelAtomImpl = (&'static str, Arc<dyn handl_ops::Op>);

/// A module's kernel impl table — what `ops::kernel_impls::*_impls()`
/// returns (P-013 T-151).
pub type KernelImplTable = fn() -> Vec<KernelAtomImpl>;

#[derive(Debug, Deserialize)]
struct RawSpec {
    /// Single-module form: `module: proc` + top-level `atoms:`.
    module: Option<String>,
    #[serde(default)]
    atoms: Vec<AtomDecl>,
    /// Multi-module form: `modules: { math: { atoms: [...] } }`.
    #[serde(default)]
    modules: BTreeMap<String, RawModule>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    #[serde(default)]
    atoms: Vec<AtomDecl>,
}

/// One atom declaration — a row of the IDL.
#[derive(Debug, Clone, Deserialize)]
pub struct AtomDecl {
    /// Qualified dispatch key, e.g. `"proc.argv"`.
    pub name: String,
    /// Handl signature text (`"Unit ~> (List String)"`); `None` =
    /// unsigned atom (dynamic-top typecheck behavior).
    #[serde(default)]
    pub sig: Option<String>,
    /// Key into the impl table (`impl:` in yaml — `impl` is a Rust
    /// keyword, hence the rename).
    #[serde(rename = "impl")]
    pub impl_key: String,
    /// Totality metadata (replaces the dead `guarantee:` clause).
    #[serde(default)]
    pub total: bool,
}

/// Parse an `atoms.yaml` document into `(module_name, atoms)` pairs.
///
/// Accepts both the single-module (`module:` + `atoms:`) and the
/// multi-module (`modules:` map) shapes; the single-module form yields
/// exactly one pair. Returns an error on malformed yaml or a document
/// declaring neither shape.
pub fn parse_atoms(yaml_text: &str) -> Result<Vec<(String, Vec<AtomDecl>)>, String> {
    let raw: RawSpec =
        serde_yaml::from_str(yaml_text).map_err(|e| format!("atoms.yaml parse: {e}"))?;
    if !raw.modules.is_empty() {
        return Ok(raw
            .modules
            .into_iter()
            .map(|(name, m)| (name, m.atoms))
            .collect());
    }
    match raw.module {
        Some(module) => Ok(vec![(module, raw.atoms)]),
        None => Err("atoms.yaml: neither `module:`+`atoms:` nor `modules:` present".to_string()),
    }
}

/// Select one module's atom declarations from a parsed spec document.
///
/// `module` is the [`crate::packages::ModuleSpec`] name (e.g. `"proc"`,
/// `"math"`). For a single-module document the declared `module:` must
/// match; for a `modules:` map the key must exist.
pub fn module_atoms(yaml_text: &str, module: &str) -> Result<Vec<AtomDecl>, String> {
    let specs = parse_atoms(yaml_text)?;
    let known: Vec<String> = specs.iter().map(|(n, _)| n.clone()).collect();
    specs
        .into_iter()
        .find(|(name, _)| name == module)
        .map(|(_, atoms)| atoms)
        .ok_or_else(|| format!("atoms.yaml: no module `{module}` (declared: {known:?})"))
}

/// Build an [`OpRegistry`] from atom declarations + an impl table.
///
/// Each atom's `impl:` key resolves to a [`Function`] impl; the atom's
/// `sig` (when present and parseable) is attached via
/// [`OpRegistry::register_with_sig`] so typecheck sees the declared
/// signature (T-148). Errors:
///
/// - `impl:` key missing from the table → error (spec/impl drift).
/// - `sig` present but unparseable → error (a spec bug must not
///   silently degrade to the dynamic top).
/// - duplicate `name` → last one wins (same replace semantics as
///   [`OpRegistry::register`]).
pub fn registry_from_atoms(atoms: &[AtomDecl], impls: &[AtomImpl]) -> Result<OpRegistry, String> {
    let mut registry = OpRegistry::new();
    for atom in atoms {
        let func = impls
            .iter()
            .find(|(key, _)| *key == atom.impl_key)
            .map(|(_, f)| Arc::clone(f))
            .ok_or_else(|| {
                format!(
                    "atoms.yaml: atom `{}`: no impl `{}` in table",
                    atom.name, atom.impl_key
                )
            })?;
        match &atom.sig {
            Some(text) => {
                let sig = sig_to_type_value(text).ok_or_else(|| {
                    format!("atoms.yaml: atom `{}`: unparseable sig `{text}`", atom.name)
                })?;
                registry.register_with_sig(&atom.name, func, sig);
            }
            None => registry.register(&atom.name, func),
        }
    }
    Ok(registry)
}

// ---------------------------------------------------------------------------
// Kernel-op variant (P-013 T-151)
// ---------------------------------------------------------------------------

/// Presents a kernel [`handl_ops::Op`] as a host [`Function`] — the same
/// adaptation [`handl_plugin_lib_handl::OpRegistry::import_kernel`] applies
/// internally, re-stated here (that adapter is private to
/// `plugin-lib-handl`) so spec-driven modules can attach per-atom sigs via
/// [`OpRegistry::register_with_sig`] — something `import_kernel` cannot do.
struct KernelOpFn(Arc<dyn handl_ops::Op>);

impl Function for KernelOpFn {
    fn call(&self, args: &[serde_json::Value], ctx: &mut EvalContext) -> Result<serde_json::Value, EvalError> {
        self.0.call(args, ctx).map_err(EvalError::from)
    }
}

/// Build an [`OpRegistry`] from atom declarations + a **kernel** impl
/// table (`handl_ops::Op` impls from `handl-lib-*`/`lib-*` crates).
///
/// Identical contract to [`registry_from_atoms`] — same spec/impl drift and
/// sig-parse errors — except each impl is adapted through [`KernelOpFn`]
/// and the atom's `sig` is attached with [`OpRegistry::register_with_sig`],
/// so `RegistrySource::KernelAtoms` modules get the same T-148 typed-op
/// surface as host-side `Atoms` modules (the old `import_kernel` path
/// carried no signatures).
pub fn registry_from_atoms_kernel(
    atoms: &[AtomDecl],
    impls: &[KernelAtomImpl],
) -> Result<OpRegistry, String> {
    let mut registry = OpRegistry::new();
    for atom in atoms {
        let op = impls
            .iter()
            .find(|(key, _)| *key == atom.impl_key)
            .map(|(_, op)| Arc::clone(op))
            .ok_or_else(|| {
                format!(
                    "atoms.yaml: atom `{}`: no impl `{}` in table",
                    atom.name, atom.impl_key
                )
            })?;
        let func: Arc<dyn Function + Send + Sync> = Arc::new(KernelOpFn(op));
        match &atom.sig {
            Some(text) => {
                let sig = sig_to_type_value(text).ok_or_else(|| {
                    format!("atoms.yaml: atom `{}`: unparseable sig `{text}`", atom.name)
                })?;
                registry.register_with_sig(&atom.name, func, sig);
            }
            None => registry.register(&atom.name, func),
        }
    }
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Echo;
    impl Function for Echo {
        fn call(
            &self,
            _args: &[serde_json::Value],
            _ctx: &mut handl_platform::EvalContext,
        ) -> Result<serde_json::Value, handl_platform::EvalError> {
            Ok(json!(null))
        }
    }

    const SPEC: &str = r#"
module: proc
atoms:
  - name: proc.argv
    sig: "Unit ~> (List String)"
    impl: proc_argv
    total: true
  - name: proc.exit
    sig: "Int ~> a"
    impl: proc_exit
  - name: proc.pid
    sig: "Unit ~> Int"
    impl: proc_pid
"#;

    fn impls() -> Vec<AtomImpl> {
        vec![
            ("proc_argv", Arc::new(Echo)),
            ("proc_exit", Arc::new(Echo)),
            ("proc_pid", Arc::new(Echo)),
        ]
    }

    #[test]
    fn parses_single_module_spec() {
        let atoms = module_atoms(SPEC, "proc").unwrap();
        assert_eq!(atoms.len(), 3);
        assert_eq!(atoms[0].name, "proc.argv");
        assert_eq!(atoms[0].impl_key, "proc_argv");
        assert!(atoms[0].total);
        assert_eq!(atoms[0].sig.as_deref(), Some("Unit ~> (List String)"));
        assert_eq!(atoms[1].sig.as_deref(), Some("Int ~> a"));
    }

    #[test]
    fn rejects_unknown_module() {
        assert!(module_atoms(SPEC, "math").is_err());
    }

    #[test]
    fn parses_multi_module_spec() {
        let yaml = r#"
modules:
  math:
    atoms:
      - { name: math.abs, sig: "Float ~> Float", impl: math_abs }
  str:
    atoms:
      - { name: str.len, impl: str_len }
"#;
        let math = module_atoms(yaml, "math").unwrap();
        assert_eq!(math.len(), 1);
        assert_eq!(math[0].name, "math.abs");
        let str_atoms = module_atoms(yaml, "str").unwrap();
        assert!(str_atoms[0].sig.is_none());
        assert!(module_atoms(yaml, "proc").is_err());
    }

    #[test]
    fn registry_from_atoms_carries_fn_and_sig() {
        let atoms = module_atoms(SPEC, "proc").unwrap();
        let registry = registry_from_atoms(&atoms, &impls()).unwrap();
        assert_eq!(registry.len(), 3);
        assert!(registry.contains("proc.argv"));
        // Sig attached → tagged type.function value.
        let sig = registry.signature("proc.argv").unwrap();
        assert_eq!(sig["__tag__"], "type.function");
        assert_eq!(sig["body"]["element"]["__tag__"], "type.string");
    }

    #[test]
    fn missing_impl_and_bad_sig_are_errors() {
        let atoms = module_atoms(SPEC, "proc").unwrap();
        assert!(registry_from_atoms(&atoms, &[]).is_err());
        let bad = vec![AtomDecl {
            name: "x.y".into(),
            sig: Some("not a ~> sig !!!".into()),
            impl_key: "proc_argv".into(),
            total: false,
        }];
        assert!(registry_from_atoms(&bad, &impls()).is_err());
    }
}
