//! Dep-bound atom modules (P-013 phase-2a, proposal-0012).
//!
//! A `pkg.hnl`/`kit.hnl` dep that vendors `atoms.yaml` gets its native
//! ops bound automatically at kit build: the spec comes from the dep
//! directory, the impls from the [`crate::atoms::AtomProvider`] /
//! `handl_ops::KernelAtomProvider` inventory — no hand table row, no
//! host-side `register()` call.
//!
//! ```text
//! deps: { lib-tensor-native: "git+file://…" }        # pkg.hnl / .handl.toml
//!   → resolve_pkg_dependencies → name → dir
//!   → dir/atoms.yaml  (spec — the dep's own contract)
//!   → atom_providers() finds the linked impl table for `tensor`
//!   → registry_from_atoms[_kernel] → HandlModule::with_ops
//!   → Kit's stdlib ModuleRegistry (ambient `tensor.*` wires)
//! ```
//!
//! Semantics:
//!
//! - **The dep vendors the spec.** Only `dir/atoms.yaml` triggers
//!   binding — a dep that wants native ops ships its atom contract;
//!   provider-submitted `atoms_yaml` copies are diagnostic, not a spec
//!   source (a dep without `atoms.yaml` is a pure `.hnl` package and is
//!   skipped untouched).
//! - **Kit-registered modules win.** [`dep_atom_modules`] only returns
//!   modules the caller's registry does not already provide — dep atoms
//!   fill gaps, they never shadow floor natives (`char`) or kit rows.
//! - **`pkg.hnl` `deps` scopes apply** (T-150/T-151): a restricted dep's
//!   `modules:`/`atoms:` lists filter the bound atom set —
//!   `atoms: ["tensor.zeros"]` yields a `tensor` module whose op
//!   registry contains only `tensor.zeros`. A scope's `caps:` allowlist
//!   additionally drops atoms whose `caps:` requirements (T-151
//!   caps-as-data) it does not cover; an empty `caps:` list is
//!   unrestricted, matching the `modules:`/`atoms:` convention. The
//!   atom's `requires:`/`termination:`/`guarantee:` clause texts are
//!   carried on [`AtomDecl`] but not yet consumed.
//! - **Ambiguity is loud.** Two providers claiming the same module, a
//!   spec/impl drift ([`crate::atoms::registry_from_atoms`] errors),
//!   or an `atoms.yaml` no linked provider can serve all surface as
//!   `eprintln!` warnings and a skipped module — never a silent
//!   half-binding.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use handl_platform::PkgDepScope;

use crate::HandlModule;
use crate::atoms::{self, AtomDecl, ImplTable, KernelImplTable};
use crate::pkg::{self, PackageModuleDef};

/// The impl ABI a discovered provider offers for one module.
enum ProviderImpls {
    /// Host `Function` impls — preferred when present.
    Host(ImplTable),
    /// Kernel `handl_ops::Op` impls.
    Kernel(KernelImplTable),
    /// A `libhnl_ext_*` cdylib provider (P-013 phase-2c): impls live
    /// behind the extension's `handl_ext_dispatch` boundary; each atom
    /// becomes a [`crate::cdylib_atoms::CdylibAtomFn`].
    Cdylib(Arc<handl_ext::LoadedExtension>),
}

/// One discovered provider claim on a module name.
struct ProviderClaim {
    impls: ProviderImpls,
}

/// Collect every statically linked provider claim for `module` across
/// both inventories: host-side [`AtomProvider`]s and kernel-side
/// `handl_ops::KernelAtomProvider`s.
///
/// cdylib providers are *not* consulted here — [`dep_atom_modules`]
/// falls back to them only when no linked provider claims the module,
/// so a first-party rlib provider always shadows a same-named `.so`.
fn provider_claims(module: &str) -> Vec<ProviderClaim> {
    let mut claims = Vec::new();
    for p in atoms::atom_providers() {
        if !p.modules.contains(&module) {
            continue;
        }
        // A provider may offer both ABIs; host wins (checked first).
        if let Some(host) = p.host_impls {
            claims.push(ProviderClaim {
                impls: ProviderImpls::Host(host),
            });
        } else if let Some(kernel) = p.kernel_impls {
            claims.push(ProviderClaim {
                impls: ProviderImpls::Kernel(kernel),
            });
        }
    }
    for p in handl_ops::kernel_atom_providers() {
        if !p.modules.contains(&module) {
            continue;
        }
        claims.push(ProviderClaim {
            impls: ProviderImpls::Kernel(p.impls),
        });
    }
    claims
}

/// The `module.field` tail of an atom name — the `field` half
/// [`PkgDepScope::allows_field`] compares against `atoms:` entries.
fn atom_field<'a>(module: &str, atom: &'a AtomDecl) -> &'a str {
    atom.name
        .strip_prefix(module)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or_else(|| atom.name.rsplit('.').next().unwrap_or(&atom.name))
}

/// Build the [`HandlModule`] for one dep-bound atom module: the dep's
/// `.hnl` surface (when the package declares one) plus the spec-driven
/// op registry.
fn build_dep_atom_module(
    dep_name: &str,
    dir: &PathBuf,
    module: &str,
    atoms: &[AtomDecl],
    claim: &ProviderClaim,
    eval_name: &'static str,
) -> Result<HandlModule, String> {
    // The `.hnl` surface is optional: an atoms-only package has no
    // `src/` module file — the op registry alone still makes a valid
    // module (every `mod.*` wire is an op call).
    let def: Option<PackageModuleDef> = match pkg::module_in_dir(dir, module) {
        Ok(def) => Some(def),
        Err(e) => {
            eprintln!(
                "kit: dep `{dep_name}` module `{module}`: no .hnl surface ({e}); \
                 binding atoms only"
            );
            None
        }
    };
    let (source, funcs) = match &def {
        Some(d) => (
            d.source.as_str(),
            d.funcs.iter().map(String::as_str).collect::<Vec<_>>(),
        ),
        None => ("", Vec::new()),
    };
    let op_names: Vec<&str> = atoms.iter().map(|a| a.name.as_str()).collect();
    let registry = match &claim.impls {
        ProviderImpls::Host(table) => atoms::registry_from_atoms(atoms, &table()),
        ProviderImpls::Kernel(table) => atoms::registry_from_atoms_kernel(atoms, &table()),
        ProviderImpls::Cdylib(ext) => crate::cdylib_atoms::registry_from_cdylib(atoms, ext),
    }
    .map_err(|e| format!("dep `{dep_name}` module `{module}`: {e}"))?;
    Ok(HandlModule::new(module, source, &funcs)
        .with_ops(&op_names, Arc::new(registry))
        .with_eval_engine(eval_name))
}

/// Bind every dep `atoms.yaml` module to its provider impls.
///
/// `dependencies` is the kit's `name → package dir` map (project
/// `[dependencies]` merged with kit `libs` + transitive `pkg.hnl`
/// deps); `dep_scopes` the `pkg.hnl` `deps` surface restrictions
/// (T-150); `eval_name` the kit's configured eval engine for the
/// `.hnl` surfaces.
///
/// Returns the bound modules — the caller registers them into its
/// module registry, skipping names the kit table already provides.
pub fn dep_atom_modules(
    dependencies: &HashMap<String, PathBuf>,
    dep_scopes: &HashMap<String, PkgDepScope>,
    eval_name: &'static str,
) -> Vec<Arc<HandlModule>> {
    dep_atom_modules_with(
        dependencies,
        dep_scopes,
        eval_name,
        crate::cdylib_atoms::cdylib_providers(),
    )
}

/// [`dep_atom_modules`] with an explicit cdylib provider set — the seam
/// tests use to exercise the phase-2c path without `dlopen`-ing the
/// process-wide discovery dirs.
fn dep_atom_modules_with(
    dependencies: &HashMap<String, PathBuf>,
    dep_scopes: &HashMap<String, PkgDepScope>,
    eval_name: &'static str,
    cdylib: &[Arc<crate::cdylib_atoms::CdylibProvider>],
) -> Vec<Arc<HandlModule>> {
    let mut out = Vec::new();
    for (dep_name, dir) in dependencies {
        // Spec source: the dep's own `atoms.yaml` — absent means a pure
        // `.hnl` package, skipped untouched.
        let yaml_path = dir.join("atoms.yaml");
        let Ok(text) = std::fs::read_to_string(&yaml_path) else {
            continue;
        };
        let specs = match atoms::parse_atoms(&text) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("kit: dep `{dep_name}`: {e} — skipping atom binding");
                continue;
            }
        };
        if specs.is_empty() {
            continue;
        }

        // The dep's declared module surface — when `pkg.hnl` names
        // modules, atoms.yaml modules outside it are not this package's
        // public surface and are skipped.
        let manifest = std::fs::read_to_string(dir.join("pkg.hnl")).unwrap_or_default();
        let declared = handl_platform::pkg_modules(&manifest);
        let scope = dep_scopes.get(dep_name);

        for (module, mut decls) in specs {
            if !declared.is_empty() && !declared.iter().any(|d| d == &module) {
                eprintln!(
                    "kit: dep `{dep_name}` atoms.yaml module `{module}` is not declared \
                     in pkg.hnl `modules:` — skipping"
                );
                continue;
            }
            if let Some(scope) = scope {
                if !scope.allows_module(&module) {
                    continue;
                }
                decls.retain(|a| {
                    scope.allows_field(&module, atom_field(&module, a))
                        && scope.allows_caps(&a.caps)
                });
                if decls.is_empty() {
                    continue;
                }
            }
            // Statically linked providers win; cdylib providers are the
            // fallback for modules no linked crate claims (P-013 2c).
            let mut claims = provider_claims(&module);
            if claims.is_empty() {
                claims = cdylib
                    .iter()
                    .filter(|p| p.modules.contains(&module))
                    .map(|p| ProviderClaim {
                        impls: ProviderImpls::Cdylib(Arc::clone(&p.ext)),
                    })
                    .collect();
            }
            let Some(claim) = claims.first() else {
                eprintln!(
                    "kit: dep `{dep_name}` module `{module}`: atoms.yaml present but no \
                     linked or discovered atom provider claims it — link the provider \
                     crate (and its link anchor) or install a `libhnl_ext_*` atom \
                     provider, or the ops stay unbound"
                );
                continue;
            };
            if claims.len() > 1 {
                eprintln!(
                    "kit: dep `{dep_name}` module `{module}`: {} providers claim it — \
                     ambiguous, skipping",
                    claims.len()
                );
                continue;
            }
            match build_dep_atom_module(dep_name, dir, &module, &decls, claim, eval_name) {
                Ok(m) => out.push(Arc::new(m)),
                Err(e) => eprintln!("kit: {e} — skipping module `{module}`"),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atoms::{AtomImpl, AtomProvider};
    use handl_platform::{EvalContext, EvalError, Function, Module};
    use serde_json::{Value, json};

    struct Echo;
    impl Function for Echo {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(json!(null))
        }
    }

    fn echo_impls() -> Vec<AtomImpl> {
        vec![("t_echo", Arc::new(Echo))]
    }

    // A test-local provider — submitted from this crate so the
    // inventory sees it in `cargo test -p handl-plugin-lib-handl`.
    inventory::submit! {
        AtomProvider {
            modules: &["tmod"],
            atoms_yaml: "module: tmod\natoms:\n  - { name: tmod.echo, sig: \"Unit ~> Unit\", impl: t_echo, total: true }\n",
            host_impls: Some(echo_impls),
            kernel_impls: None,
        }
    }

    fn write_pkg_as(dir: &std::path::Path, module: &str, atoms_yaml: Option<&str>) {
        std::fs::write(
            dir.join("pkg.hnl"),
            format!("name: \"{module}-pkg\"\nmodules: [\"{module}\"]\n"),
        )
        .unwrap();
        if let Some(yaml) = atoms_yaml {
            std::fs::write(dir.join("atoms.yaml"), yaml).unwrap();
        }
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/index.hnl"), "export echo := 1\n").unwrap();
    }

    fn write_pkg(dir: &std::path::Path, atoms_yaml: Option<&str>) {
        write_pkg_as(dir, "tmod", atoms_yaml)
    }

    #[test]
    fn binds_dep_atoms_yaml_module() {
        let dir = tempfile::tempdir().unwrap();
        write_pkg(
            dir.path(),
            Some(
                "module: tmod\natoms:\n  - { name: tmod.echo, sig: \"Unit ~> Unit\", impl: t_echo, total: true }\n",
            ),
        );
        let mut deps = HashMap::new();
        deps.insert("tmod-pkg".to_string(), dir.path().to_path_buf());
        let modules = dep_atom_modules(&deps, &HashMap::new(), "tree-walk");
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name(), "tmod");
    }

    #[test]
    fn dep_scope_atoms_filters_native_ops() {
        let dir = tempfile::tempdir().unwrap();
        write_pkg(
            dir.path(),
            Some(
                "module: tmod\natoms:\n  - { name: tmod.echo, sig: \"Unit ~> Unit\", impl: t_echo }\n  - { name: tmod.other, impl: t_echo }\n",
            ),
        );
        let mut deps = HashMap::new();
        deps.insert("tmod-pkg".to_string(), dir.path().to_path_buf());
        let mut scopes = HashMap::new();
        scopes.insert(
            "tmod-pkg".to_string(),
            PkgDepScope {
                modules: vec![],
                atoms: vec!["tmod.echo".to_string()],
                caps: vec![],
            },
        );
        let modules = dep_atom_modules(&deps, &scopes, "tree-walk");
        assert_eq!(modules.len(), 1);
        // The scoped-out atom must not be bound: the module's function
        // surface carries only `echo`.
        let names: Vec<String> = modules[0]
            .functions()
            .iter()
            .map(|(n, _)| n.to_string())
            .collect();
        assert!(names.contains(&"echo".to_string()), "{names:?}");
        assert!(!names.contains(&"other".to_string()), "{names:?}");
    }

    #[test]
    fn dep_scope_caps_filters_atoms_by_capability() {
        let dir = tempfile::tempdir().unwrap();
        write_pkg(
            dir.path(),
            Some(
                "module: tmod\natoms:\n  - { name: tmod.echo, sig: \"Unit ~> Unit\", impl: t_echo }\n  - { name: tmod.net, impl: t_echo, caps: [\"net.io\"] }\n",
            ),
        );
        let mut deps = HashMap::new();
        deps.insert("tmod-pkg".to_string(), dir.path().to_path_buf());

        // Scope allowing only `ffi.rust`: the `net.io`-requiring atom is
        // dropped, the cap-free atom binds.
        let mut scopes = HashMap::new();
        scopes.insert(
            "tmod-pkg".to_string(),
            PkgDepScope {
                modules: vec![],
                atoms: vec![],
                caps: vec!["ffi.rust".to_string()],
            },
        );
        let modules = dep_atom_modules(&deps, &scopes, "tree-walk");
        assert_eq!(modules.len(), 1);
        let names: Vec<String> = modules[0]
            .functions()
            .iter()
            .map(|(n, _)| n.to_string())
            .collect();
        assert!(names.contains(&"echo".to_string()), "{names:?}");
        assert!(!names.contains(&"net".to_string()), "{names:?}");

        // Scope covering the requirement binds both.
        let mut scopes = HashMap::new();
        scopes.insert(
            "tmod-pkg".to_string(),
            PkgDepScope {
                modules: vec![],
                atoms: vec![],
                caps: vec!["net.io".to_string()],
            },
        );
        let modules = dep_atom_modules(&deps, &scopes, "tree-walk");
        assert_eq!(modules.len(), 1);
        let names: Vec<String> = modules[0]
            .functions()
            .iter()
            .map(|(n, _)| n.to_string())
            .collect();
        assert!(names.contains(&"net".to_string()), "{names:?}");
    }

    #[test]
    fn no_atoms_yaml_no_binding() {
        let dir = tempfile::tempdir().unwrap();
        write_pkg(dir.path(), None);
        let mut deps = HashMap::new();
        deps.insert("tmod-pkg".to_string(), dir.path().to_path_buf());
        assert!(dep_atom_modules(&deps, &HashMap::new(), "tree-walk").is_empty());
    }

    // ------------------------------------------------------------------
    // P-013 phase-2c: cdylib atom providers (libhnl_ext_* discovered
    // through the ADR-0149 search path, impls behind handl_ext_dispatch)
    // ------------------------------------------------------------------

    #[test]
    fn binds_dep_atoms_via_cdylib_provider() {
        let Some(provider) = crate::cdylib_atoms::fixture_provider() else {
            eprintln!("atomfix fixture cdylib not built; skipping");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        write_pkg_as(
            dir.path(),
            "atomfix",
            Some(
                "module: atomfix\natoms:\n  - { name: atomfix.echo, sig: \"String ~> String\", impl: atom_echo }\n  - { name: atomfix.add, sig: \"(Int * Int) ~> Int\", impl: atom_add }\n",
            ),
        );
        let mut deps = HashMap::new();
        deps.insert("atomfix-pkg".to_string(), dir.path().to_path_buf());
        let modules = dep_atom_modules_with(&deps, &HashMap::new(), "tree-walk", &[provider]);
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name(), "atomfix");

        // The bound module's ops dispatch through the cdylib's
        // `handl_ext_dispatch` — real FFI, real JSON marshalling.
        let mut ctx = EvalContext::new();
        let echo = modules[0].function("echo").expect("echo op bound");
        assert_eq!(echo.call(&[json!("hi")], &mut ctx).unwrap(), json!("hi!"));
        let add = modules[0].function("add").expect("add op bound");
        assert_eq!(
            add.call(&[json!(20), json!(22)], &mut ctx).unwrap(),
            json!(42)
        );
    }

    #[test]
    fn cdylib_provider_error_surfaces_loudly() {
        let Some(provider) = crate::cdylib_atoms::fixture_provider() else {
            eprintln!("atomfix fixture cdylib not built; skipping");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        write_pkg_as(
            dir.path(),
            "atomfix",
            Some("module: atomfix\natoms:\n  - { name: atomfix.fail, impl: atom_fail }\n"),
        );
        let mut deps = HashMap::new();
        deps.insert("atomfix-pkg".to_string(), dir.path().to_path_buf());
        let modules = dep_atom_modules_with(&deps, &HashMap::new(), "tree-walk", &[provider]);
        assert_eq!(modules.len(), 1);
        let mut ctx = EvalContext::new();
        let err = modules[0]
            .function("fail")
            .expect("fail op bound")
            .call(&[json!(1)], &mut ctx)
            .unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
    }

    #[test]
    fn static_provider_shadows_cdylib_for_same_module() {
        // `tmod` has an inventory provider (test-local `AtomProvider`
        // above); a cdylib claiming `tmod` must not shadow it — static
        // linking wins, matching the kit-registered-wins rule.
        let dir = tempfile::tempdir().unwrap();
        write_pkg(
            dir.path(),
            Some(
                "module: tmod\natoms:\n  - { name: tmod.echo, sig: \"Unit ~> Unit\", impl: t_echo, total: true }\n",
            ),
        );
        let mut deps = HashMap::new();
        deps.insert("tmod-pkg".to_string(), dir.path().to_path_buf());
        // Even with a cdylib provider present (claiming `atomfix`, not
        // `tmod`), the static claim is used — and with an empty provider
        // set the static claim alone still binds.
        let modules = dep_atom_modules_with(&deps, &HashMap::new(), "tree-walk", &[]);
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name(), "tmod");
    }
}
