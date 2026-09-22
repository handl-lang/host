//! Post-passes over the emitted module: drop records/typedefs not
//! referenced by emitted functions, and repair `rec:` fields left
//! pointing at records that were skipped or pruned.

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

/// Degrade `rec:` field ports to `Handle` when the nested record is not
/// among the emitted records (skipped by config, or pruned above — e.g.
/// glibc's `__atomic_wide_counter` union, which has no standalone decl).
/// A dangling port would emit a reference to a record that does not
/// exist; the ABI stays right via the field's byte size, the same
/// fallback `record_fields_descriptor` uses for unknown nested records.
pub(crate) fn fix_dangling_rec_fields(module: &mut SdkgenModule) {
    let emitted: HashSet<String> = module.records.iter().map(|r| r.name.clone()).collect();
    for r in &mut module.records {
        for f in &mut r.fields {
            if let Some(rn) = f.descriptor.strip_prefix("rec:") {
                if !emitted.contains(rn) {
                    f.port = "Handle".into();
                }
            }
        }
    }
}
