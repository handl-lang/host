//! cdylib atom providers (P-013 phase-2c, proposal-0012).
//!
//! Phase-2a binds dep `atoms.yaml` specs to impl tables submitted through
//! the `inventory` crate — which only sees crates *linked into the host
//! artifact*. Phase-2c adds the dynamic half: a third-party/user atom
//! provider ships as a `libhnl_ext_*` shared object discovered through
//! the ADR-0149 extension search path (`--ext-dir` is a host-CLI concern;
//! libraries see `$HANDL_EXT_DIR` → `~/.handl/ext/` → dev `<exe>` dirs via
//! [`handl_ext::discover`]).
//!
//! An extension opts in by setting
//! [`ExtensionManifest::atoms_yaml`](handl_ext::ExtensionManifest::atoms_yaml)
//! in its manifest; the host parses it for the module names the provider
//! claims. Atom *calls* cross the ordinary `handl_ext_dispatch` boundary —
//! `cmd` is the qualified atom name, `args[0]` a JSON array of call args,
//! `stdout` the JSON result (see `handl_ext::atoms` for the convention and
//! the provider-side helpers).
//!
//! Each atom becomes a [`CdylibAtomFn`] — a host [`Function`] that
//! marshals args to JSON, dispatches, and decodes the result — so the
//! rest of the dep-binding pipeline (`registry_from_atoms`-equivalent
//! signature attachment, scope filtering, kit-registered-wins) is
//! identical to statically linked providers.
//!
//! Honest limits (proposal-0012 §d): an FFI boundary per atom call, no
//! [`EvalContext`](handl_platform::EvalContext) across it (providers run
//! with a fresh context — atoms needing host recursion cannot be
//! cdylib providers), and provider `.so`/host ABI version skew is the
//! provider's problem. Static linking stays the default for first-party
//! providers; this path exists for providers that cannot be linked.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use handl_ext::{ExtRequest, LoadedExtension};
use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value;

use crate::atoms::AtomDecl;
use crate::{OpRegistry, sig_to_type_value};

/// A `libhnl_ext_*` shared library discovered as an atom provider.
///
/// `modules` is the set of `atoms.yaml` module keys the provider's own
/// spec claims — matching is by module name, same rule as the inventory
/// providers (dep name and module name are different namespaces).
pub struct CdylibProvider {
    /// The loaded extension; kept alive for the process lifetime so the
    /// dispatch pointers inside [`CdylibAtomFn`] stay valid.
    pub(crate) ext: Arc<LoadedExtension>,
    /// Module names claimed via the manifest's `atoms_yaml`.
    pub(crate) modules: BTreeSet<String>,
}

/// Scan `ext_dirs` for `libhnl_ext_*` libraries whose manifest carries
/// `atoms_yaml`, returning one [`CdylibProvider`] per provider.
///
/// `ext_dirs` maps to `handl_ext::discover`'s `--ext-dir` slots; an empty
/// slice takes the env/home/dev chain. Extensions that fail to parse
/// their own `atoms.yaml` are warned about and skipped — a broken
/// provider must not take down kit build.
fn discover_cdylib_providers(ext_dirs: &[PathBuf]) -> Vec<Arc<CdylibProvider>> {
    let discovery = handl_ext::discover(ext_dirs);
    let mut out = Vec::new();
    for ext in discovery.extensions {
        let Some(yaml) = ext.manifest().atoms_yaml.clone() else {
            continue;
        };
        let modules: BTreeSet<String> = match crate::atoms::parse_atoms(&yaml) {
            Ok(specs) => specs.into_iter().map(|(name, _)| name).collect(),
            Err(e) => {
                eprintln!(
                    "kit: atom-provider extension `{}` ({}): {e} — ignored",
                    ext.manifest().name,
                    ext.path().display()
                );
                continue;
            }
        };
        if modules.is_empty() {
            continue;
        }
        out.push(Arc::new(CdylibProvider {
            ext: Arc::new(ext),
            modules,
        }));
    }
    out
}

/// The process-wide cdylib provider set.
///
/// Discovery `dlopen`s every `libhnl_ext_*` candidate, so it runs once
/// per process and is cached. Only invoked when a dep `atoms.yaml`
/// actually needs a provider no statically linked crate claims.
pub(crate) fn cdylib_providers() -> &'static [Arc<CdylibProvider>] {
    static PROVIDERS: OnceLock<Vec<Arc<CdylibProvider>>> = OnceLock::new();
    PROVIDERS.get_or_init(|| discover_cdylib_providers(&[]))
}

/// Build an [`OpRegistry`] whose entries dispatch through `ext`.
///
/// Mirrors [`crate::atoms::registry_from_atoms`]: each atom's `sig`
/// attaches via `register_with_sig` (unparseable sig = spec bug, loud
/// error), absent sig = unsigned atom.
pub(crate) fn registry_from_cdylib(
    atoms: &[AtomDecl],
    ext: &Arc<LoadedExtension>,
) -> Result<OpRegistry, String> {
    let mut registry = OpRegistry::new();
    for atom in atoms {
        let func: Arc<dyn Function + Send + Sync> = Arc::new(CdylibAtomFn {
            ext: Arc::clone(ext),
            atom: atom.name.clone(),
        });
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

/// A host [`Function`] that forwards one atom call to a cdylib provider.
///
/// Wire format (`handl_ext::atoms`): `dispatch(atom_name, ExtRequest {
/// args: [<json args array>], cwd, env })`; `code == 0` → `stdout` is the
/// JSON result, otherwise `stderr` is the failure.
struct CdylibAtomFn {
    ext: Arc<LoadedExtension>,
    /// Qualified atom name — sent verbatim as the dispatch `cmd`.
    atom: String,
}

impl Function for CdylibAtomFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let payload = serde_json::to_string(args).map_err(|e| {
            EvalError::Other(format!("atom `{}`: args encode failed: {e}", self.atom))
        })?;
        let request = ExtRequest {
            args: vec![payload],
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            env: std::env::vars().collect(),
        };
        let response = self.ext.dispatch(&self.atom, &request).map_err(|e| {
            EvalError::Other(format!(
                "atom `{}`: provider dispatch failed: {e}",
                self.atom
            ))
        })?;
        if response.code != 0 {
            let detail = if response.stderr.trim().is_empty() {
                format!("provider exited with code {}", response.code)
            } else {
                response.stderr.trim().to_string()
            };
            return Err(EvalError::Other(format!("atom `{}`: {detail}", self.atom)));
        }
        serde_json::from_str(&response.stdout).map_err(|e| {
            EvalError::Other(format!(
                "atom `{}`: provider result is not valid JSON: {e}",
                self.atom
            ))
        })
    }
}

/// Locate the fixture cdylib `libhnl_ext_atomfix.*` built into the
/// workspace `target/<profile>` dir (the dev-dep builds it for tests).
#[cfg(test)]
fn fixture_path() -> Option<PathBuf> {
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())?
        .join("target");
    // `cargo build -p <fixture>` drops the cdylib in `target/debug/`;
    // building it as a dev-dep drops it in `target/debug/deps/` — check
    // both (the latter is also the dev-scan `<exe>` dir for test bins).
    for dir in [target.join("debug"), target.join("debug").join("deps")] {
        for name in [
            "libhnl_ext_atomfix.so",
            "libhnl_ext_atomfix.dylib",
            "hnl_ext_atomfix.dll",
        ] {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Load the fixture extension, or `None` when the cdylib was not built.
#[cfg(test)]
pub(crate) fn load_fixture() -> Option<Arc<LoadedExtension>> {
    let path = fixture_path()?;
    match LoadedExtension::load(&path) {
        Ok(ext) => Some(Arc::new(ext)),
        Err(e) => {
            eprintln!("fixture load failed: {e}");
            None
        }
    }
}

/// A [`CdylibProvider`] wrapping the fixture extension — the explicit
/// provider set `dep_atom_modules_with` tests inject.
#[cfg(test)]
pub(crate) fn fixture_provider() -> Option<Arc<CdylibProvider>> {
    let ext = load_fixture()?;
    Some(Arc::new(CdylibProvider {
        ext,
        modules: BTreeSet::from(["atomfix".to_string()]),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn manifest_declares_atomfix_provider() {
        let Some(ext) = load_fixture() else {
            eprintln!("atomfix fixture cdylib not built; skipping");
            return;
        };
        let yaml = ext.manifest().atoms_yaml.as_deref().expect("atoms_yaml");
        let specs = crate::atoms::parse_atoms(yaml).expect("provider atoms.yaml parses");
        let modules: Vec<String> = specs.iter().map(|(m, _)| m.clone()).collect();
        assert_eq!(modules, vec!["atomfix".to_string()]);
    }

    #[test]
    fn registry_dispatches_atom_calls_through_ffi() {
        let Some(ext) = load_fixture() else {
            eprintln!("atomfix fixture cdylib not built; skipping");
            return;
        };
        let atoms =
            crate::atoms::module_atoms(ext.manifest().atoms_yaml.as_deref().unwrap(), "atomfix")
                .unwrap();
        let registry = registry_from_cdylib(&atoms, &ext).unwrap();
        assert!(registry.contains("atomfix.echo"));
        assert!(registry.contains("atomfix.add"));

        let func = registry.get("atomfix.echo").unwrap();
        let mut ctx = EvalContext::new();
        let out = func.call(&[json!("hello")], &mut ctx).unwrap();
        assert_eq!(out, json!("hello!"));

        let func = registry.get("atomfix.add").unwrap();
        let out = func.call(&[json!(20), json!(22)], &mut ctx).unwrap();
        assert_eq!(out, json!(42));
    }

    #[test]
    fn provider_error_surfaces_as_eval_error() {
        let Some(ext) = load_fixture() else {
            eprintln!("atomfix fixture cdylib not built; skipping");
            return;
        };
        let atoms = vec![AtomDecl {
            name: "atomfix.fail".to_string(),
            sig: None,
            impl_key: "atomfix_fail".to_string(),
            total: false,
            caps: vec![],
            requires: vec![],
            termination: vec![],
            guarantee: vec![],
        }];
        let registry = registry_from_cdylib(&atoms, &ext).unwrap();
        let func = registry.get("atomfix.fail").unwrap();
        let mut ctx = EvalContext::new();
        let err = func.call(&[json!(1)], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
    }
}
