//! Doc layering for the emitted module: `.sdkgen.yaml` `docs:` overrides
//! and an optional external [`DocSource`] over comments captured from
//! the header.

use handl_plugin_sdkgen::{DocSource, SdkgenModule, pick_doc};

use crate::backend::CBackend;
use crate::backend::man::ManPageDocSource;

impl CBackend {
    /// Layer `.sdkgen.yaml` `docs:` overrides and an optional external
    /// [`DocSource`] (e.g. [`ManPageDocSource`]) over comments
    /// captured from the header. Per slot: override > in-source comment
    /// > external lookup. Keys are C symbol names; record fields use
    /// `record.field`.
    ///
    /// When `external_docs` is not `false` (the default) and no `source`
    /// is supplied, a [`ManPageDocSource`] is auto-created and consulted.
    pub fn apply_docs(&self, module: &mut SdkgenModule, source: Option<&dyn DocSource>) {
        let docs = &self.config.docs;
        // Respect `external_docs: false` — skip external lookup entirely.
        // Otherwise auto-create a man-page source if the caller gave none.
        let fallback = if self.config.external_docs != Some(false) && source.is_none() {
            Some(ManPageDocSource::new())
        } else {
            None
        };
        let src: Option<&dyn DocSource> = if self.config.external_docs == Some(false) {
            None
        } else {
            source.or_else(|| fallback.as_ref().map(|s| s as &dyn DocSource))
        };
        for f in &mut module.functions {
            pick_doc(&mut f.doc, docs, &[&f.name], src, Some(&f.name));
        }
        for r in &mut module.records {
            pick_doc(&mut r.doc, docs, &[&r.name], src, Some(&r.name));
            for f in &mut r.fields {
                let key = format!("{}.{}", r.name, f.name);
                pick_doc(&mut f.doc, docs, &[&key], None, None);
            }
        }
        for t in &mut module.typedefs {
            pick_doc(&mut t.doc, docs, &[&t.name], src, Some(&t.name));
        }
        for c in &mut module.enum_constants {
            pick_doc(&mut c.doc, docs, &[&c.name], None, None);
        }
    }
}
