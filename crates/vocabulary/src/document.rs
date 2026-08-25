/// Classification of a key in a handl document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// The top-level field declarations key (`$` in the canonical dialect).
    Field,
    /// Computed expression key (`expr`).
    Expr,
    /// Argument type key (`arg`).
    Arg,
    /// Merge flag key (`merge`).
    Merge,
    /// Inheritance flag key (`inheritable`).
    Inheritable,
    /// Mutability flag key (`mutable`).
    Mutable,
    /// Function parameters key (`params`).
    Params,
    /// Function return schema key (`return`).
    Return,
    /// Function body key (`body`).
    Body,
    /// Alias key (`as`).
    Alias,
    /// A regular, non-special key.
    Ordinary,
}

/// In-process plugin interface for document vocabularies.
pub trait DocumentVocabularyPlugin: Send + Sync {
    /// Returns `true` if `key` is a special/reserved document key.
    fn is_special_key(&self, key: &str) -> bool;

    /// Classifies `key` into one of the known document key kinds.
    fn classify_key(&self, key: &str) -> KeyKind;

    /// Returns `true` if `key` may not be used as a user-defined field name.
    fn is_reserved(&self, key: &str) -> bool;
}
