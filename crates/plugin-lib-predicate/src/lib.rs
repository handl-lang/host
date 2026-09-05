//! # handl-plugin-lib-predicate
//!
//! Generic predicate combinators for the handl platform.
//!
//! Provides a small, reusable predicate language: boolean combinators
//! (`and`, `or`, `not`), atomic predicates over a value, and string/path
//! matchers (exact, glob). Consumers such as the test runner layer their
//! own domain-specific fields (`tag`, `path`, `kind`, `status`) on top by
//! supplying closures to [`Predicate::atom`].
//!
//! This crate is the generic home for the predicate/filter concern
//! identified in ADR-0089 *Generic infrastructure dependencies*. The test
//! runner is the first consumer.

#![warn(missing_docs)]

use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

/// A predicate over values of type `T`.
pub enum Predicate<T> {
    /// Always true.
    True,
    /// Always false.
    False,
    /// An atomic predicate implemented as a closure.
    Atom(Arc<dyn Fn(&T) -> bool + Send + Sync>),
    /// True iff all sub-predicates are true.
    And(Vec<Predicate<T>>),
    /// True iff any sub-predicate is true.
    Or(Vec<Predicate<T>>),
    /// Negation of a sub-predicate.
    Not(Box<Predicate<T>>),
}

impl<T> Clone for Predicate<T> {
    fn clone(&self) -> Self {
        match self {
            Predicate::True => Predicate::True,
            Predicate::False => Predicate::False,
            Predicate::Atom(f) => Predicate::Atom(Arc::clone(f)),
            Predicate::And(ps) => Predicate::And(ps.clone()),
            Predicate::Or(ps) => Predicate::Or(ps.clone()),
            Predicate::Not(p) => Predicate::Not(p.clone()),
        }
    }
}

impl<T> Predicate<T> {
    /// Evaluate the predicate against `item`.
    pub fn eval(&self, item: &T) -> bool {
        match self {
            Predicate::True => true,
            Predicate::False => false,
            Predicate::Atom(f) => f(item),
            Predicate::And(ps) => ps.iter().all(|p| p.eval(item)),
            Predicate::Or(ps) => ps.iter().any(|p| p.eval(item)),
            Predicate::Not(p) => !p.eval(item),
        }
    }

    /// Combine with another predicate using logical AND.
    pub fn and(self, other: Predicate<T>) -> Predicate<T> {
        match (self, other) {
            (Predicate::True, p) | (p, Predicate::True) => p,
            (Predicate::False, _) | (_, Predicate::False) => Predicate::False,
            (Predicate::And(mut ps), Predicate::And(mut qs)) => {
                ps.append(&mut qs);
                Predicate::And(ps)
            }
            (Predicate::And(mut ps), q) | (q, Predicate::And(mut ps)) => {
                ps.push(q);
                Predicate::And(ps)
            }
            (p, q) => Predicate::And(vec![p, q]),
        }
    }

    /// Combine with another predicate using logical OR.
    pub fn or(self, other: Predicate<T>) -> Predicate<T> {
        match (self, other) {
            (Predicate::False, p) | (p, Predicate::False) => p,
            (Predicate::True, _) | (_, Predicate::True) => Predicate::True,
            (Predicate::Or(mut ps), Predicate::Or(mut qs)) => {
                ps.append(&mut qs);
                Predicate::Or(ps)
            }
            (Predicate::Or(mut ps), q) | (q, Predicate::Or(mut ps)) => {
                ps.push(q);
                Predicate::Or(ps)
            }
            (p, q) => Predicate::Or(vec![p, q]),
        }
    }

    /// Negate the predicate.
    pub fn not(self) -> Predicate<T> {
        match self {
            Predicate::True => Predicate::False,
            Predicate::False => Predicate::True,
            Predicate::Not(p) => *p,
            p => Predicate::Not(Box::new(p)),
        }
    }

    /// Create an atomic predicate from a closure.
    pub fn atom<F>(f: F) -> Predicate<T>
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        Predicate::Atom(Arc::new(f))
    }
}

impl<T> Default for Predicate<T> {
    fn default() -> Self {
        Predicate::True
    }
}

impl<T> Debug for Predicate<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Predicate::True => write!(f, "True"),
            Predicate::False => write!(f, "False"),
            Predicate::Atom(_) => write!(f, "Atom(...)"),
            Predicate::And(ps) => write!(f, "And({:?})", ps),
            Predicate::Or(ps) => write!(f, "Or({:?})", ps),
            Predicate::Not(p) => write!(f, "Not({:?})", p),
        }
    }
}

/// Match a string against a simple glob pattern.
///
/// Supports:
/// - `?` — matches any single character
/// - `*` — matches any sequence of characters within one path segment
/// - `**` — matches any sequence of characters including path separators
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    // For flat string matching, treat '**' the same as '*'.
    // path_matches handles the path-separator-aware semantics.
    let pattern = pattern.replace("**", "*");
    glob_matches_segment(&pattern, value)
}

fn glob_matches_segment(pattern: &str, value: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();
    let mut value_chars = value.chars().peekable();

    while let Some(p) = pattern_chars.next() {
        match p {
            '*' => {
                if pattern_chars.peek().is_none() {
                    return true;
                }
                let remaining_pattern: String = pattern_chars.collect();
                let remaining_value: String = value_chars.clone().collect();
                for i in 0..=remaining_value.len() {
                    if glob_matches_segment(&remaining_pattern, &remaining_value[i..]) {
                        return true;
                    }
                }
                return false;
            }
            '?' => {
                if value_chars.next().is_none() {
                    return false;
                }
            }
            c => {
                if value_chars.next() != Some(c) {
                    return false;
                }
            }
        }
    }

    value_chars.next().is_none()
}

/// Match a path against a glob pattern.
///
/// The pattern is split on `/` and matched against the path's components.
/// A `**` component matches any number of nested directories.
pub fn path_matches(pattern: &str, path: &Path) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<Option<&str>> = path.components().map(|c| c.as_os_str().to_str()).collect();

    path_matches_parts(&pattern_parts, &path_parts)
}

fn path_matches_parts(pattern: &[&str], path: &[Option<&str>]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((pat, rest)) if *pat == "**" => {
            if rest.is_empty() {
                return true;
            }
            for i in 0..=path.len() {
                if path_matches_parts(rest, &path[i..]) {
                    return true;
                }
            }
            false
        }
        Some((pat, rest)) => match path.split_first() {
            None => false,
            Some((Some(part), path_rest)) => {
                glob_matches_segment(pat, part) && path_matches_parts(rest, path_rest)
            }
            Some((None, _)) => false,
        },
    }
}

/// A matcher for string values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringMatcher {
    /// Exact string match.
    Exact(String),
    /// Glob pattern match.
    Glob(String),
}

impl StringMatcher {
    /// Create an exact matcher.
    pub fn exact<S: Into<String>>(value: S) -> Self {
        StringMatcher::Exact(value.into())
    }

    /// Create a glob matcher.
    pub fn glob<S: Into<String>>(pattern: S) -> Self {
        StringMatcher::Glob(pattern.into())
    }

    /// Test whether the matcher accepts `value`.
    pub fn matches(&self, value: &str) -> bool {
        match self {
            StringMatcher::Exact(s) => s == value,
            StringMatcher::Glob(pattern) => glob_matches(pattern, value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestItem {
        tag: &'static str,
    }

    #[test]
    fn true_and_false() {
        let item = TestItem { tag: "x" };
        assert!(Predicate::<TestItem>::True.eval(&item));
        assert!(!Predicate::<TestItem>::False.eval(&item));
    }

    #[test]
    fn atom_evaluates_closure() {
        let p = Predicate::atom(|item: &TestItem| item.tag == "unit");
        let item = TestItem { tag: "unit" };
        assert!(p.eval(&item));
    }

    #[test]
    fn and_or_not_combinators() {
        let is_unit = Predicate::atom(|item: &TestItem| item.tag == "unit");
        let is_fast = Predicate::atom(|item: &TestItem| item.tag == "fast");
        let item = TestItem { tag: "unit" };

        assert!(is_unit.clone().and(is_fast.clone().not()).eval(&item));
        assert!(!is_unit.clone().and(is_fast.clone()).eval(&item));
        assert!(is_unit.clone().or(is_fast.clone()).eval(&item));
    }

    #[test]
    fn glob_matching() {
        assert!(glob_matches("*.hnl", "foo.hnl"));
        assert!(!glob_matches("*.hnl", "foo.txt"));
        assert!(glob_matches("test_*", "test_foo"));
        assert!(glob_matches("a?c", "abc"));
        assert!(!glob_matches("a?c", "abbc"));
    }

    #[test]
    fn path_matching() {
        assert!(path_matches("src/**/*.hnl", Path::new("src/foo/bar.hnl")));
        assert!(!path_matches("src/**/*.hnl", Path::new("test/foo.hnl")));
        assert!(path_matches("tests/*.hnl", Path::new("tests/foo.hnl")));
        assert!(!path_matches(
            "tests/*.hnl",
            Path::new("tests/nested/foo.hnl")
        ));
    }

    #[test]
    fn string_matcher() {
        assert!(StringMatcher::exact("unit").matches("unit"));
        assert!(!StringMatcher::exact("unit").matches("Unit"));
        assert!(StringMatcher::glob("u*").matches("unit"));
        assert!(!StringMatcher::glob("u*").matches("fast"));
    }
}
