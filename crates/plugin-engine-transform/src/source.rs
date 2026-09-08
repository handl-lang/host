//! Rule-source seam for transform engines (ADR-0094).

use handl_language::{Language, TransformDef};

/// A source of [`TransformDef`]s for a given pass.
///
/// The default implementation is [`LanguageRuleSource`], which reads from
/// `lang.transforms`. Alternate implementations can load rules from a codemod
/// file or an extra YAML fragment set without merging them into the
/// [`Language`].
///
/// Rule sources are shared across threads by the parallel engine, so the
/// trait is `Send + Sync`.
pub trait RuleSource: Send + Sync {
    /// Return all transform rules for `pass`, sorted by priority ascending.
    ///
    /// The default source returns owned clones because a [`RuleSource`] may
    /// synthesize rules that do not live in a `FragmentSet`.
    fn rules_for(&self, lang: &Language, pass: &str) -> Vec<TransformDef>;
}

/// The default rule source: reads the `transforms` fragment set from a
/// [`Language`].
#[derive(Debug, Clone, Copy, Default)]
pub struct LanguageRuleSource;

impl LanguageRuleSource {
    /// Create a new language-backed rule source.
    pub fn new() -> Self {
        Self
    }
}

impl RuleSource for LanguageRuleSource {
    fn rules_for(&self, lang: &Language, pass: &str) -> Vec<TransformDef> {
        let mut rules: Vec<TransformDef> = lang
            .transforms
            .iter()
            .filter(|d| d.pass == pass)
            .cloned()
            .collect();
        rules.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.id.cmp(&b.id)));
        rules
    }
}
