//! Per-pass dispatch table (ADR-0094 §4).
//!
//! Built once per pass from the engine's [`RuleSource`]: a
//! `HashMap<kind, Vec<TransformDef>>` gives one string lookup per node, with
//! each bucket sorted by `priority` ascending (first match wins). A
//! `HashMap<id, TransformDef>` index supports [`RewriteStep::Apply`] and
//! surgical `ApplyAt` rewrites.

use std::collections::HashMap;

use handl_language::{Language, TransformDef};
use handl_plugin_engine_transform::source::RuleSource;

/// The dispatch table for a single pass.
///
/// `by_kind` maps a node kind to its rules in fire order (priority
/// ascending, then id — the [`RuleSource`] contract). `by_id` maps rule ids
/// to their definitions for [`RewriteStep::Apply`](handl_language::RewriteStep::Apply)
/// lookups.
pub(crate) struct Dispatch {
    by_kind: HashMap<String, Vec<TransformDef>>,
    by_id: HashMap<String, TransformDef>,
}

impl Dispatch {
    /// Build the dispatch table for `pass` from `source`.
    pub(crate) fn build(source: &dyn RuleSource, lang: &Language, pass: &str) -> Self {
        let mut by_kind: HashMap<String, Vec<TransformDef>> = HashMap::new();
        let mut by_id = HashMap::new();
        for def in source.rules_for(lang, pass) {
            by_id.insert(def.id.clone(), def.clone());
            by_kind.entry(def.node_kind.clone()).or_default().push(def);
        }
        for rules in by_kind.values_mut() {
            rules.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.id.cmp(&b.id)));
        }
        Self { by_kind, by_id }
    }

    /// The rules dispatching on `kind`, in fire order.
    pub(crate) fn rules_for(&self, kind: &str) -> Option<&[TransformDef]> {
        self.by_kind.get(kind).map(|v| v.as_slice())
    }

    /// Look up a rule by its stable id.
    pub(crate) fn rule_by_id(&self, id: &str) -> Option<&TransformDef> {
        self.by_id.get(id)
    }
}
