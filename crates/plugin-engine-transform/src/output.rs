//! Output and provenance types for the transform operation (ADR-0094).

use std::collections::BTreeMap;
use std::time::Duration;

use handl_language::{Ast, AstRef};

/// The result of a successful transform.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformOutput {
    /// The output AST (rewritten in a fresh arena).
    pub ast: Ast,
    /// The root node in the output AST.
    pub root: AstRef,
    /// Provenance trace: input-node → output-node for each rewrite.
    pub trace: RewriteTrace,
    /// Pass statistics for observability.
    pub stats: PassStats,
}

impl TransformOutput {
    /// Create a new output value.
    pub fn new(ast: Ast, root: AstRef, trace: RewriteTrace, stats: PassStats) -> Self {
        Self {
            ast,
            root,
            trace,
            stats,
        }
    }
}

/// A single input-node to output-node rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    /// The input node reference.
    pub from: AstRef,
    /// The output node reference.
    pub to: AstRef,
    /// The id of the rule that performed the rewrite.
    pub rule_id: String,
}

/// Provenance trace for a transform.
///
/// Records, for each rewritten node, the input [`AstRef`] and the output
/// [`AstRef`] plus the rule id. This is free: `AstRef`s are stable arena
/// indices, so tracing is a `Vec` append.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RewriteTrace {
    /// The rewrites performed during the pass.
    pub rewrites: Vec<Rewrite>,
}

impl RewriteTrace {
    /// Create an empty trace.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up the output [`AstRef`] for an input `from` node.
    ///
    /// Returns `None` if `from` was never rewritten.
    pub fn new_ref(&self, from: AstRef) -> Option<AstRef> {
        self.rewrites.iter().find(|r| r.from == from).map(|r| r.to)
    }
}

/// Statistics for a single transform pass.
#[derive(Debug, Clone, PartialEq)]
pub struct PassStats {
    /// Number of nodes visited.
    pub visited: usize,
    /// Number of nodes rewritten.
    pub rewritten: usize,
    /// Number of rewrites per rule id.
    pub per_rule: BTreeMap<String, usize>,
    /// Wall-clock time spent in the pass.
    pub duration: Duration,
}

impl PassStats {
    /// Create empty pass statistics.
    pub fn new() -> Self {
        Self {
            visited: 0,
            rewritten: 0,
            per_rule: BTreeMap::new(),
            duration: Duration::from_secs(0),
        }
    }
}

impl Default for PassStats {
    fn default() -> Self {
        Self::new()
    }
}
