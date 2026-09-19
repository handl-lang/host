//! Post-pass: drop records/typedefs not referenced by emitted functions.

use handl_plugin_sdkgen::SdkgenModule;
use std::collections::HashSet;

/// Retain only records and typedefs named (as port types) by an emitted
/// function — its `out_struct`, argument ports, or return port. No-op
/// when no functions were emitted (a types-only generation run).
pub(crate) fn prune_unreferenced(module: &mut SdkgenModule) {
    if module.functions.is_empty() {
        return;
    }
    let mut referenced: HashSet<String> = HashSet::new();
    for f in &module.functions {
        if let Some(r) = &f.out_struct {
            referenced.insert(r.clone());
        }
        for a in &f.port_args {
            referenced.insert(a.port.clone());
        }
        referenced.insert(f.return_port.clone());
    }
    module.records.retain(|r| referenced.contains(&r.name));
    module.typedefs.retain(|t| referenced.contains(&t.name));
}
