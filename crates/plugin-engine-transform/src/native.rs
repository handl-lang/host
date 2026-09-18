//! Native-rewrite seam and helper context (ADR-0094).
//!
//! [`RewriteCtx`] bundles the five pieces a native rewrite needs — the
//! [`Language`] (for role-based shape queries), the input node (pre-remap),
//! the input arena, the output arena, and the old→new `remap` table — so a
//! native can move existing arena-building code (e.g. the legacy
//! `handl-runtime::desugar_pass` desugar functions) behind the
//! [`NativeRewrite`] trait almost verbatim.

use std::collections::HashMap;

use handl_language::{Ast, AstNode, AstRef, LangError, Language};
use serde_json::Value;

/// A registry of native rewrites: Rust functions that interpret
/// `TransformRule::Native` rules.
///
/// Natives receive a [`RewriteCtx`] and return the new output [`AstRef`]. They
/// never touch raw arena indices directly; child remapping and node allocation
/// go through the context.
///
/// Native registries are shared across threads by the parallel engine, so the
/// trait is `Send + Sync`.
pub trait NativeRewrite: Send + Sync {
    /// Apply the native rewrite named `name` to the input node in `ctx`.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure (unknown native, bad child shape,
    /// allocation error, etc.).
    fn rewrite(&self, name: &str, ctx: &mut RewriteCtx<'_>) -> Result<AstRef, LangError>;
}

/// Helper context passed to native rewrites.
///
/// Wraps the [`Language`], the input node (pre-remap), the input arena, the
/// output arena, and the old-to-new index `remap` table. It exposes the same
/// remapping primitives used by the legacy `handl-runtime::desugar_pass`, so
/// existing native desugar functions can move here almost verbatim in Phase 3.
///
/// The shared-reference accessors ([`node`](Self::node), [`ast`](Self::ast),
/// [`lang`](Self::lang), [`remap`](Self::remap)) return references with the
/// context's `'a` lifetime, so they do not keep the context borrowed while the
/// output arena is mutated through [`out`](Self::out) or the `push_*` helpers.
pub struct RewriteCtx<'a> {
    lang: &'a Language,
    node: &'a AstNode,
    node_ref: AstRef,
    ast: &'a Ast,
    out: &'a mut Ast,
    remap: &'a [Option<usize>],
    declined: bool,
}

impl<'a> RewriteCtx<'a> {
    /// Create a new rewrite context.
    pub fn new(
        lang: &'a Language,
        node: &'a AstNode,
        node_ref: AstRef,
        ast: &'a Ast,
        out: &'a mut Ast,
        remap: &'a [Option<usize>],
    ) -> Self {
        Self {
            lang,
            node,
            node_ref,
            ast,
            out,
            remap,
            declined: false,
        }
    }

    /// Whether the native declined to rewrite via [`pass_through`](Self::pass_through).
    /// The engine uses this to keep a decline out of the rewrite trace and
    /// per-rule stats — a declined node is unchanged, not a match site.
    pub fn declined(&self) -> bool {
        self.declined
    }

    /// The [`Language`] the pass runs against (for role-based shape queries).
    pub fn lang(&self) -> &'a Language {
        self.lang
    }

    /// The input node (pre-remap).
    pub fn node(&self) -> &'a AstNode {
        self.node
    }

    /// The input arena position of the node being rewritten. Natives that
    /// need to locate the node relative to the rest of the tree (e.g. to
    /// detect that they are inside a quoted subterm) use this.
    pub fn node_ref(&self) -> AstRef {
        self.node_ref
    }

    /// The input arena.
    pub fn ast(&self) -> &'a Ast {
        self.ast
    }

    /// The output arena (mutable reborrow).
    pub fn out(&mut self) -> &mut Ast {
        self.out
    }

    /// The old→new index remap table.
    pub fn remap(&self) -> &'a [Option<usize>] {
        self.remap
    }

    /// The remapped new arena index of child `i`.
    ///
    /// Returns `None` if the child does not exist or was not remapped.
    pub fn remap_child(&self, i: usize) -> Option<usize> {
        self.child_ref(i)
            .and_then(|idx| self.remap.get(idx).copied().flatten())
    }

    /// The remapped new arena indices of all children from `start` onward.
    pub fn remap_children_from(&self, start: usize) -> Vec<usize> {
        let children = self.node.payload.get("children").and_then(|c| c.as_array());
        match children {
            Some(arr) => arr
                .iter()
                .skip(start)
                .filter_map(|v| v.as_u64().map(|i| i as usize))
                .filter_map(|idx| self.remap.get(idx).copied().flatten())
                .collect(),
            None => Vec::new(),
        }
    }

    /// Push a leaf node (no children) into the output arena.
    ///
    /// Returns the new output arena index.
    pub fn push_leaf(&mut self, kind: &str, payload: Value) -> usize {
        self.out
            .push(AstNode::new(kind).with_payload(payload))
            .index()
    }

    /// Push a node with children into the output arena.
    ///
    /// Returns the new output arena index.
    pub fn push_node(&mut self, kind: &str, children: Vec<usize>) -> usize {
        let payload = if children.is_empty() {
            Value::Null
        } else {
            serde_json::json!({ "children": children })
        };
        self.out
            .push(AstNode::new(kind).with_payload(payload))
            .index()
    }

    /// Push the input node through unchanged, with its `children` array
    /// remapped into the output arena.
    ///
    /// This is the "decline" path for natives that decide — after inspecting
    /// the node — not to rewrite it (mirroring the legacy `try_rust_desugar`
    /// `None` return, which fell through to pushing the remapped node). It
    /// marks the context [`declined`](Self::declined) so the engine does not
    /// record a rewrite for an unchanged node.
    pub fn pass_through(&mut self) -> AstRef {
        self.declined = true;
        let mut payload = self.node.payload.clone();
        if let Some(children) = payload.get_mut("children").and_then(|c| c.as_array_mut()) {
            for child in children.iter_mut() {
                if let Some(old_idx) = child.as_u64() {
                    if let Some(Some(new_idx)) = self.remap.get(old_idx as usize) {
                        *child = Value::from(*new_idx);
                    }
                }
            }
        }
        self.out.push(AstNode {
            kind: self.node.kind.clone(),
            span: self.node.span,
            payload,
        })
    }

    /// Whether child `i` has the given `kind` in the input arena.
    pub fn child_is_kind(&self, i: usize, kind: &str) -> bool {
        self.child_ref(i)
            .and_then(|idx| self.ast.get(AstRef::new(idx)))
            .map_or(false, |child| child.kind == kind)
    }

    /// The original input arena index of child `i`.
    ///
    /// Returns `None` if the child does not exist or is not a valid index.
    pub fn child_ref(&self, i: usize) -> Option<usize> {
        self.node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.get(i))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
    }
}

/// A native-rewrite registry backed by a `HashMap` of named functions.
///
/// The stored closures are `Send + Sync` so the registry can be shared across
/// threads by the parallel engine.
pub struct NativeRewriteRegistry {
    fns: HashMap<
        String,
        Box<dyn Fn(&mut RewriteCtx<'_>) -> Result<AstRef, LangError> + Send + Sync>,
    >,
}

impl NativeRewriteRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            fns: HashMap::new(),
        }
    }

    /// Register a native rewrite under `name`.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        f: Box<dyn Fn(&mut RewriteCtx<'_>) -> Result<AstRef, LangError> + Send + Sync>,
    ) {
        self.fns.insert(name.into(), f);
    }

    /// Whether a native rewrite is registered under `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.fns.contains_key(name)
    }
}

impl NativeRewrite for NativeRewriteRegistry {
    fn rewrite(&self, name: &str, ctx: &mut RewriteCtx<'_>) -> Result<AstRef, LangError> {
        match self.fns.get(name) {
            Some(f) => f(ctx),
            None => Err(LangError::engine(format!("unknown native rewrite: {name}"))),
        }
    }
}
