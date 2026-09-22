//! Post-pass: declare `types/c/<header>` imports on the module.
//!
//! Collects the ctypes aliases referenced by the surviving port types,
//! grouped by submodule, so the emitter can render
//! `from "types/c/<header>" import {...}` — declared on the module as
//! data, keeping the emitter backend-agnostic.

use handl_plugin_sdkgen::{ModuleImport, SdkgenModule};
use std::collections::{BTreeMap, BTreeSet};

use crate::map::ctypes_module;

/// Append one [`ModuleImport`] per `types/c` submodule naming the ctypes
/// aliases `module`'s port types reference.
pub(crate) fn collect_type_imports(module: &mut SdkgenModule) {
    let mut by_module: BTreeMap<&str, BTreeSet<String>> = Default::default();
    let mut note = |port: &str| {
        if let Some(m) = ctypes_module(port) {
            by_module.entry(m).or_default().insert(port.to_string());
        }
    };
    for f in &module.functions {
        for a in &f.port_args {
            note(&a.port);
        }
        note(&f.return_port);
    }
    for r in &module.records {
        for fl in &r.fields {
            note(&fl.port);
        }
    }
    for t in &module.typedefs {
        note(&t.port);
    }
    for (package, names) in by_module {
        module.imports.push(ModuleImport {
            package: package.into(),
            names: names.into_iter().collect(),
        });
    }
}
