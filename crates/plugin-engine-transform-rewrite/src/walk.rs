//! The arena walker (ADR-0094 §4).
//!
//! Generalizes `apply_desugar` from `handl-runtime::desugar_pass`: a
//! `remap: Vec<Option<usize>>` old→new index table, copy-on-write
//! construction into a fresh output arena, and per-node dispatch through a
//! [`Dispatch`] table — one `HashMap` lookup per node, rules tried in
//! priority order, first [`when`](handl_language::TransformDef::when)-matching
//! rule fires.
//!
//! Ordering guarantee preserved from the legacy pass: a kind that has *any*
//! rule in the dispatch table never consults the native fallback — a
//! declarative rule whose `when` fails produces a pass-through, not a native
//! rewrite (today's `desugar` → `try_rust_desugar` ordering).
//!
//! Traversals:
//!
//! - **Bottom-up**: insertion-order iteration (children before parents in
//!   the Pratt arena layout), the proven `apply_desugar` algorithm.
//! - **Top-down**: recursive pre-order descent from the root; a rule fires
//!   on a node before its kept children are visited (`Keep`-family steps
//!   resolve children lazily). Native rules and pass-through nodes force
//!   children-first processing so the `remap` table is populated before a
//!   [`RewriteCtx`] consults it.
//! - **Fixpoint { max }**: repeated bottom-up passes until an iteration
//!   performs no rewrite, or `max` iterations / [`MAX_NODES`] is exceeded.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, TransformDef, TransformRule};
use handl_plugin_engine_transform::native::{NativeRewrite, RewriteCtx};
use handl_plugin_engine_transform::output::{PassStats, Rewrite, RewriteTrace};

use crate::condition::matches_condition;
use crate::dispatch::Dispatch;
use crate::step::{self, Resolver};

/// Hard node-count budget for [`Traversal::Fixpoint`](crate::Traversal)
/// iterations — a growing rewrite that never converges is cut off here
/// rather than exhausting memory.
pub const MAX_NODES: usize = 1_000_000;

/// The per-kind native fallback consulted when no declarative rule exists
/// for a kind: `(pass, node_kind) → native_name`.
pub(crate) type NativeFallback = HashMap<(String, String), String>;

/// Remap the `children` array in a node's payload from old to new indices.
///
/// Verbatim from the legacy `desugar_pass`: every pass-through node is a
/// clone of the input node with its children rewritten through `remap`.
fn remap_children(node: &AstNode, remap: &[Option<usize>]) -> AstNode {
    let mut new_payload = node.payload.clone();
    if let Some(children) = new_payload
        .get_mut("children")
        .and_then(|c| c.as_array_mut())
    {
        for child in children.iter_mut() {
            if let Some(old_idx) = child.as_u64() {
                if let Some(Some(new_idx)) = remap.get(old_idx as usize) {
                    *child = serde_json::Value::from(*new_idx);
                }
            }
        }
    }
    AstNode {
        kind: node.kind.clone(),
        span: node.span,
        payload: new_payload,
    }
}

/// Mutable state for a single pass over one input arena.
pub(crate) struct PassRun<'a> {
    lang: &'a Language,
    ast: &'a Ast,
    /// The pass name (native-fallback lookup key).
    pass: &'a str,
    /// The dispatch table for this pass (borrowed — built once per pass).
    dispatch: &'a Dispatch,
    /// The wired native-rewrite registry, if any.
    natives: Option<Arc<dyn NativeRewrite>>,
    /// The `(pass, kind) → native name` fallback table.
    fallback: &'a NativeFallback,
    /// `true` for top-down traversal (children resolved lazily).
    topdown: bool,
    /// When set, only this rule fires, and only at this input index
    /// (`ApplyAt` surgical mode).
    surgical: Option<(usize, &'a TransformDef)>,
    /// Old→new index table.
    remap: Vec<Option<usize>>,
    /// Cycle guard for the recursive top-down visit.
    visiting: HashSet<usize>,
    /// Provenance trace.
    pub(crate) trace: RewriteTrace,
    /// Pass statistics.
    pub(crate) stats: PassStats,
}

impl<'a> PassRun<'a> {
    pub(crate) fn new(
        lang: &'a Language,
        ast: &'a Ast,
        pass: &'a str,
        dispatch: &'a Dispatch,
        natives: Option<Arc<dyn NativeRewrite>>,
        fallback: &'a NativeFallback,
        topdown: bool,
        surgical: Option<(usize, &'a TransformDef)>,
    ) -> Self {
        Self {
            lang,
            ast,
            pass,
            dispatch,
            natives,
            fallback,
            topdown,
            surgical,
            remap: vec![None; ast.len()],
            visiting: HashSet::new(),
            trace: RewriteTrace::new(),
            stats: PassStats::new(),
        }
    }

    /// Replace the old→new remap table with a pre-populated one.
    pub(crate) fn set_remap(&mut self, remap: Vec<Option<usize>>) {
        self.remap = remap;
    }

    /// Bottom-up pass: walk the whole arena in insertion order (children
    /// before parents — the Pratt convention).
    pub(crate) fn run_bottom_up(&mut self, out: &mut Ast) -> Result<(), LangError> {
        for i in 0..self.ast.len() {
            let node = match self.ast.get(AstRef::new(i)) {
                Some(n) => n,
                None => continue,
            };
            self.stats.visited += 1;
            self.process(i, node, out)?;
        }
        Ok(())
    }

    /// Top-down pass: recursive pre-order descent from `root`.
    pub(crate) fn run_top_down(&mut self, out: &mut Ast, root: AstRef) -> Result<(), LangError> {
        self.visit(root.index(), out).map(|_| ())
    }

    /// Dispatch one input node (bottom-up): fire the first matching rule,
    /// else consult the native fallback, else push the remapped node.
    pub(crate) fn process(&mut self, i: usize, node: &AstNode, out: &mut Ast) -> Result<(), LangError> {
        let remapped = remap_children(node, &self.remap);
        if let Some((at, def)) = self.surgical {
            // Surgical mode: only `def` fires, only at `at`.
            if i == at {
                self.check_when(def, i, node)?;
                let new_idx = self.fire(def, i, node, out)?;
                self.remap[i] = Some(new_idx);
                return Ok(());
            }
        } else if let Some(rules) = self.dispatch.rules_for(&node.kind) {
            // First matching rule wins; a `when` miss produces a
            // pass-through (never the native fallback — preserving the
            // legacy declarative-before-native ordering).
            for def in rules {
                if self.rule_fires(def, node) {
                    let new_idx = self.fire(def, i, node, out)?;
                    self.remap[i] = Some(new_idx);
                    return Ok(());
                }
            }
        } else if let Some(name) = self.fallback_name(&node.kind) {
            // Native fallback: only when no declarative rule exists for the
            // kind (mirrors `try_rust_desugar`'s position in the legacy
            // pass). A declining native pushes the remapped node itself via
            // `RewriteCtx::pass_through`.
            let new_idx = self
                .fire_native(&name, node, out)
                .map_err(|e| self.annotate(e, Some(name.as_str()), Some(node)))?;
            self.remap[i] = Some(new_idx);
            self.record(i, new_idx, &format!("native:{name}"));
            return Ok(());
        }
        let new_ref = out.push(remapped);
        self.remap[i] = Some(new_ref.index());
        Ok(())
    }

    /// Recursive pre-order visit (top-down).
    ///
    /// Rules fire on a node before its children are visited; `Keep`-family
    /// steps resolve children lazily through [`Resolver::resolve`], which
    /// re-enters [`visit`](Self::visit). Native rules and pass-through nodes
    /// visit their children first so `remap` is populated before use.
    fn visit(&mut self, idx: usize, out: &mut Ast) -> Result<usize, LangError> {
        if let Some(m) = self.remap.get(idx).copied().flatten() {
            return Ok(m);
        }
        if !self.visiting.insert(idx) {
            let node = self.ast.get(AstRef::new(idx));
            return Err(self.annotate(
                LangError::engine(format!("transform cycle detected at node {idx}")),
                None,
                node,
            ));
        }
        let result = self.visit_inner(idx, out);
        self.visiting.remove(&idx);
        result
    }

    fn visit_inner(&mut self, idx: usize, out: &mut Ast) -> Result<usize, LangError> {
        let node = self
            .ast
            .get(AstRef::new(idx))
            .ok_or_else(|| {
                LangError::engine(format!(
                    "pass '{}': invalid node index {idx}",
                    self.pass
                ))
            })?;
        self.stats.visited += 1;

        if let Some((at, def)) = self.surgical {
            if idx == at {
                self.check_when(def, idx, node)?;
                self.visit_children(node, out)?;
                let new_idx = self.fire(def, idx, node, out)?;
                self.remap[idx] = Some(new_idx);
                return Ok(new_idx);
            }
        } else if let Some(rules) = self.dispatch.rules_for(&node.kind) {
            for def in rules {
                if !self.rule_fires(def, node) {
                    continue;
                }
                if matches!(def.rule, TransformRule::Native(_)) {
                    // Natives read `remap` — visit children first.
                    self.visit_children(node, out)?;
                }
                let new_idx = self.fire(def, idx, node, out)?;
                self.remap[idx] = Some(new_idx);
                return Ok(new_idx);
            }
        } else if let Some(name) = self.fallback_name(&node.kind) {
            self.visit_children(node, out)?;
            let new_idx = self
                .fire_native(&name, node, out)
                .map_err(|e| self.annotate(e, Some(name.as_str()), Some(node)))?;
            self.remap[idx] = Some(new_idx);
            self.record(idx, new_idx, &format!("native:{name}"));
            return Ok(new_idx);
        }
        // No rule fired: visit children, then push the remapped node.
        self.visit_children(node, out)?;
        let remapped = remap_children(node, &self.remap);
        let new_idx = out.push(remapped).index();
        self.remap[idx] = Some(new_idx);
        Ok(new_idx)
    }

    /// Visit every child of `node` (input-arena indices), filling `remap`.
    fn visit_children(&mut self, node: &AstNode, out: &mut Ast) -> Result<(), LangError> {
        let indices: Vec<usize> = node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|i| i as usize))
                    .collect()
            })
            .unwrap_or_default();
        for old_idx in indices {
            self.visit(old_idx, out)?;
        }
        Ok(())
    }

    /// The `(pass, kind)` native-fallback entry for `kind`, if any.
    fn fallback_name(&self, kind: &str) -> Option<String> {
        self.fallback
            .get(&(self.pass.to_string(), kind.to_string()))
            .cloned()
    }

    /// Whether `def`'s `when` condition holds on `node` (absent = fires).
    fn rule_fires(&self, def: &TransformDef, node: &AstNode) -> bool {
        match &def.when {
            Some(cond) => matches_condition(self.lang, self.ast, node, cond),
            None => true,
        }
    }

    /// Enforce `def`'s `when` for surgical application (a mismatch is an
    /// error, not a silent pass-through).
    fn check_when(&self, def: &TransformDef, idx: usize, node: &AstNode) -> Result<(), LangError> {
        if !self.rule_fires(def, node) {
            return Err(self.annotate(
                LangError::engine(format!(
                    "rule '{}' condition does not hold at node {}",
                    def.id, idx
                )),
                Some(def.id.as_str()),
                Some(node),
            ));
        }
        Ok(())
    }

    /// Attach `{pass, rule_id, node_kind, span}` context to a transform
    /// failure (ADR-0094 "Errors & diagnostics").
    ///
    /// The pass and rule id are folded into the `message` (so `.contains()`
    /// matching keeps working); `span`/`node_kind` are attached as the
    /// structured fields of [`LangError::Engine`], taken from the error when
    /// it already carries them (e.g. a native that built its own
    /// `engine_node` error) and otherwise from the node being rewritten.
    fn annotate(
        &self,
        err: LangError,
        rule_id: Option<&str>,
        node: Option<&AstNode>,
    ) -> LangError {
        let (msg, span, kind) = match err {
            LangError::Engine {
                message,
                span,
                node_kind,
                ..
            } => (message, span, node_kind),
            other => (other.to_string(), None, None),
        };
        let rule = rule_id.unwrap_or("<none>");
        LangError::engine_at(
            format!("pass '{}' rule '{}': {}", self.pass, rule, msg),
            span.or_else(|| node.and_then(|n| n.span)),
            Some(
                kind.or_else(|| node.map(|n| n.kind.clone()))
                    .unwrap_or_default(),
            ),
        )
    }

    /// Fire a matched rule on input node `i`, recording trace and stats.
    fn fire(
        &mut self,
        def: &'a TransformDef,
        i: usize,
        node: &AstNode,
        out: &mut Ast,
    ) -> Result<usize, LangError> {
        let new_idx = self
            .fire_untraced(def, node, out)
            .map_err(|e| self.annotate(e, Some(def.id.as_str()), Some(node)))?;
        self.record(i, new_idx, &def.id);
        Ok(new_idx)
    }

    /// Fire a rule without recording trace/stats (the public entry points
    /// record; [`RewriteStep::Apply`](handl_language::RewriteStep::Apply)
    /// records against the applied node itself).
    fn fire_untraced(
        &mut self,
        def: &'a TransformDef,
        node: &AstNode,
        out: &mut Ast,
    ) -> Result<usize, LangError> {
        match &def.rule {
            TransformRule::Declarative {
                target_kind,
                children,
                payload,
            } => {
                let lang = self.lang;
                let ast = self.ast;
                let r = step::build_rewrite(
                    self,
                    lang,
                    target_kind,
                    children,
                    payload.as_ref(),
                    node,
                    ast,
                    out,
                )?;
                Ok(r.index())
            }
            TransformRule::Native(name) => self.fire_native(name, node, out),
        }
    }

    /// Invoke a native rewrite through the wired registry.
    ///
    /// A `Native` rule that matches with no registry wired is a
    /// [`LangError`] — never silently skipped.
    fn fire_native(
        &mut self,
        name: &str,
        node: &AstNode,
        out: &mut Ast,
    ) -> Result<usize, LangError> {
        let natives = match &self.natives {
            Some(n) => n.clone(),
            None => {
                return Err(LangError::engine(format!(
                    "transform rule requires native rewrite '{name}' but no native registry is wired"
                )));
            }
        };
        let mut ctx = RewriteCtx::new(self.lang, node, self.ast, out, &self.remap);
        let new_ref = natives.rewrite(name, &mut ctx)?;
        Ok(new_ref.index())
    }

    /// Record a rewrite in the trace and per-rule stats.
    fn record(&mut self, from: usize, to: usize, rule_id: &str) {
        self.stats.rewritten += 1;
        *self.stats.per_rule.entry(rule_id.to_string()).or_insert(0) += 1;
        self.trace.rewrites.push(Rewrite {
            from: AstRef::new(from),
            to: AstRef::new(to),
            rule_id: rule_id.to_string(),
        });
    }

    /// Resolve the input `root` to its output [`AstRef`].
    pub(crate) fn output_root(&self, root: AstRef, out: &Ast) -> AstRef {
        self.remap
            .get(root.index())
            .copied()
            .flatten()
            .map(AstRef::new)
            .unwrap_or_else(|| AstRef::new(out.len().saturating_sub(1)))
    }
}

impl<'a> Resolver for PassRun<'a> {
    fn resolve(&mut self, out: &mut Ast, old_idx: usize) -> Result<Option<usize>, LangError> {
        if self.topdown {
            // Lazily visit the child (pre-order: the parent rule fired
            // before its kept children are rewritten).
            return self.visit(old_idx, out).map(Some);
        }
        Ok(self.remap.get(old_idx).copied().flatten())
    }

    fn apply(
        &mut self,
        out: &mut Ast,
        rule_id: &str,
        old_idx: usize,
    ) -> Result<Option<usize>, LangError> {
        let def = self
            .dispatch
            .rule_by_id(rule_id)
            .ok_or_else(|| {
                LangError::engine(format!("Apply references unknown rule '{rule_id}'"))
            })?;
        let node = match self.ast.get(AstRef::new(old_idx)) {
            Some(n) => n,
            None => return Ok(None),
        };
        if !self.rule_fires(def, node) {
            // The named rule declines: keep the child as-is.
            return self.resolve(out, old_idx);
        }
        if self.topdown || matches!(def.rule, TransformRule::Native(_)) {
            // Ensure the child's own subtree is resolved first so that
            // `remap` is populated for the rewrite's `Keep` steps and for
            // any `RewriteCtx` the native consults.
            self.visit_or_process_children(node, out)?;
        }
        let new_idx = self.fire_untraced(def, node, out)?;
        self.record(old_idx, new_idx, &def.id);
        Ok(Some(new_idx))
    }
}

impl<'a> PassRun<'a> {
    /// In `Apply`/top-down paths, make sure every child of `node` has been
    /// resolved so `remap` lookups inside the rewrite succeed.
    fn visit_or_process_children(
        &mut self,
        node: &AstNode,
        out: &mut Ast,
    ) -> Result<(), LangError> {
        if self.topdown {
            return self.visit_children(node, out);
        }
        // Bottom-up: children are already remapped by insertion order.
        Ok(())
    }
}
