//! [`MockFactory`] — a configurable factory for test token kinds.
//!
//! Implements [`EofFactory`] and holds configurable closures for building
//! token kinds. The per-concern token traits (`NumberToken`, `StringToken`,
//! `IdentToken`, `OperatorToken`, `CommentToken`, `KeywordToken`) are
//! implemented in each respective `plugin-lexer-*` crate's test module,
//! keeping this crate free of dependencies on the plugin crates.

use super::MockKind;
use handl_plugin_lexer::EofFactory;
use std::sync::Arc;

pub struct MockFactory {
    string_fn: Arc<dyn Fn(String) -> MockKind + Send + Sync>,
    number_fn: Arc<dyn Fn(f64) -> MockKind + Send + Sync>,
    ident_fn: Arc<dyn Fn(String) -> MockKind + Send + Sync>,
    comment_fn: Arc<dyn Fn(String) -> MockKind + Send + Sync>,
    keyword_fn: Arc<dyn Fn(String) -> Option<MockKind> + Send + Sync>,
    operator_fn: Arc<dyn Fn(String) -> Option<MockKind> + Send + Sync>,
}

impl Default for MockFactory {
    fn default() -> Self {
        MockFactory::new()
    }
}

impl MockFactory {
    pub fn new() -> Self {
        MockFactory {
            string_fn: Arc::new(|v| MockKind::Str(v)),
            number_fn: Arc::new(|v| MockKind::Num(v)),
            ident_fn: Arc::new(|t| MockKind::Ident(t)),
            comment_fn: Arc::new(|t| MockKind::Comment(t)),
            keyword_fn: Arc::new(|_| None),
            operator_fn: Arc::new(|_| None),
        }
    }

    pub fn with_string_kind<F>(self, f: F) -> Self
    where
        F: Fn(String) -> MockKind + Send + Sync + 'static,
    {
        MockFactory {
            string_fn: Arc::new(f),
            ..self
        }
    }

    pub fn with_number_kind<F>(self, f: F) -> Self
    where
        F: Fn(f64) -> MockKind + Send + Sync + 'static,
    {
        MockFactory {
            number_fn: Arc::new(f),
            ..self
        }
    }

    pub fn with_ident_kind<F>(self, f: F) -> Self
    where
        F: Fn(String) -> MockKind + Send + Sync + 'static,
    {
        MockFactory {
            ident_fn: Arc::new(f),
            ..self
        }
    }

    pub fn with_comment_kind<F>(self, f: F) -> Self
    where
        F: Fn(String) -> MockKind + Send + Sync + 'static,
    {
        MockFactory {
            comment_fn: Arc::new(f),
            ..self
        }
    }

    pub fn with_keyword_kind<F>(self, f: F) -> Self
    where
        F: Fn(String) -> Option<MockKind> + Send + Sync + 'static,
    {
        MockFactory {
            keyword_fn: Arc::new(f),
            ..self
        }
    }

    pub fn with_operator_kind<F>(self, f: F) -> Self
    where
        F: Fn(String) -> Option<MockKind> + Send + Sync + 'static,
    {
        MockFactory {
            operator_fn: Arc::new(f),
            ..self
        }
    }

    /// Returns the string-kind constructor closure.
    pub fn string_kind_fn(&self) -> &(dyn Fn(String) -> MockKind + Send + Sync) {
        self.string_fn.as_ref()
    }

    /// Returns the number-kind constructor closure.
    pub fn number_kind_fn(&self) -> &(dyn Fn(f64) -> MockKind + Send + Sync) {
        self.number_fn.as_ref()
    }

    /// Returns the ident-kind constructor closure.
    pub fn ident_kind_fn(&self) -> &(dyn Fn(String) -> MockKind + Send + Sync) {
        self.ident_fn.as_ref()
    }

    /// Returns the comment-kind constructor closure.
    pub fn comment_kind_fn(&self) -> &(dyn Fn(String) -> MockKind + Send + Sync) {
        self.comment_fn.as_ref()
    }

    /// Returns the keyword-kind constructor closure.
    pub fn keyword_kind_fn(&self) -> &(dyn Fn(String) -> Option<MockKind> + Send + Sync) {
        self.keyword_fn.as_ref()
    }

    /// Returns the operator-kind constructor closure.
    pub fn operator_kind_fn(&self) -> &(dyn Fn(String) -> Option<MockKind> + Send + Sync) {
        self.operator_fn.as_ref()
    }
}

impl Clone for MockFactory {
    fn clone(&self) -> Self {
        MockFactory {
            string_fn: Arc::clone(&self.string_fn),
            number_fn: Arc::clone(&self.number_fn),
            ident_fn: Arc::clone(&self.ident_fn),
            comment_fn: Arc::clone(&self.comment_fn),
            keyword_fn: Arc::clone(&self.keyword_fn),
            operator_fn: Arc::clone(&self.operator_fn),
        }
    }
}

impl EofFactory<MockKind> for MockFactory {
    fn eof_kind(&self) -> MockKind {
        MockKind::Eof
    }
}
