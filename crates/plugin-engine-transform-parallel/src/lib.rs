//! # handl-plugin-engine-transform-parallel
//!
//! A parallel [`TransformEngine`] implementation (ADR-0094) that partitions a
//! bottom-up pass at the root's top-level children. Each child subtree is
//! independent for desugar-style rules (they never rewrite across subtree
//! boundaries), so they can be rewritten in parallel. The root itself is
//! processed sequentially once every child has been remapped into a merged
//! output arena.
//!
//! The implementation uses `std::thread::scope` and an `Arc`-shared
//! [`NativeRewrite`] registry; the abstraction crate makes both `NativeRewrite`
//! and [`RuleSource`] `Send + Sync` so that [`RewriteEngine`] can be cloned and
//! moved into worker threads.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use handl_language::{Ast, AstNode, AstRef, ChildRoleSpec, LangError, Language};
use handl_plugin_engine_transform::native::NativeRewrite;
use handl_plugin_engine_transform::output::{PassStats, Rewrite, RewriteTrace, TransformOutput};
use handl_plugin_engine_transform::request::{TransformMode, TransformRequest};
use handl_plugin_engine_transform::{Match, TransformEngine};
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::Value;

/// A parallel bottom-up transform engine.
///
/// Partitions a pass at the root's immediate children, rewrites each group of
/// subtrees in parallel, and then processes the root as a sequential tail on a
/// merged arena. Small inputs fall back to the sequential [`RewriteEngine`].
#[derive(Clone)]
pub struct ParallelEngine {
    inner: RewriteEngine,
    workers: usize,
    threshold: usize,
}

impl ParallelEngine {
    /// Create a parallel engine with the default number of workers.
    pub fn new() -> Self {
        Self {
            inner: RewriteEngine::new(),
            workers: thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .max(1),
            threshold: 64,
        }
    }

    /// Set the number of parallel workers.
    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers.max(1);
        self
    }

    /// Set the node-count threshold below which the pass runs sequentially.
    ///
    /// The default is 64. The overhead of partitioning and merging is not worth
    /// it for tiny arenas.
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold.max(1);
        self
    }

    /// Wire the desugar native registry.
    pub fn with_desugar_natives(mut self) -> Self {
        self.inner = self.inner.with_desugar_natives();
        self
    }

    /// Wire a custom native-rewrite registry.
    pub fn with_native_registry(mut self, natives: Arc<dyn NativeRewrite>) -> Self {
        self.inner = self.inner.with_native_registry(natives);
        self
    }

    /// Run a single named pass, parallelizing across top-level children.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] when any worker fails or the root tail fails.
    pub fn run_pass(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        pass: &str,
    ) -> Result<TransformOutput, LangError> {
        let started = Instant::now();
        let children = child_refs(ast, root);
        if ast.len() < self.threshold || self.workers < 2 || children.len() < 2 {
            return self.inner.run_pass(lang, ast, root, pass);
        }

        let workers = self.workers.min(children.len()).max(1);
        let chunk_size = (children.len() + workers - 1) / workers;

        let groups: Vec<Vec<(usize, TransformOutput, Vec<usize>)>> = thread::scope(|s| -> Result<Vec<Vec<(usize, TransformOutput, Vec<usize>)>>, LangError> {
            let mut handles = Vec::new();
            for chunk in children.chunks(chunk_size) {
                let local = self.inner.clone();
                let chunk: Vec<usize> = chunk.to_vec();
                let handle = s.spawn(move || -> Result<Vec<(usize, TransformOutput, Vec<usize>)>, LangError> {
                    let mut results = Vec::new();
                    for &old_idx in &chunk {
                        let (sub, new_to_old) = extract_subtree(ast, AstRef::new(old_idx))?;
                        let sub_root = AstRef::new(sub.len() - 1);
                        let sub_out = local.run_pass(lang, &sub, sub_root, pass)?;
                        results.push((old_idx, sub_out, new_to_old));
                    }
                    Ok(results)
                });
                handles.push(handle);
            }

            let mut groups: Vec<Vec<(usize, TransformOutput, Vec<usize>)>> = Vec::new();
            for h in handles {
                let result = h
                    .join()
                    .map_err(|_| LangError::engine("transform worker thread panicked".to_string()))?;
                groups.push(result?);
            }
            Ok(groups)
        })?;

        let mut out = Ast::new();
        let mut child_remap = vec![None; ast.len()];
        let mut trace = RewriteTrace::new();
        let mut stats = PassStats::new();

        for group in groups {
            for (old_idx, mut sub_out, new_to_old) in group {
                let base = out.len();
                offset_ast(&mut sub_out.ast, base, lang);
                for node in sub_out.ast.iter() {
                    out.push(node.clone());
                }

                child_remap[old_idx] = Some(base + sub_out.root.index());

                for r in sub_out.trace.rewrites {
                    let from_global = new_to_old[r.from.index()];
                    let to_global = base + r.to.index();
                    trace.rewrites.push(Rewrite {
                        from: AstRef::new(from_global),
                        to: AstRef::new(to_global),
                        rule_id: r.rule_id,
                    });
                }

                stats.visited += sub_out.stats.visited;
                stats.rewritten += sub_out.stats.rewritten;
                for (k, v) in sub_out.stats.per_rule {
                    *stats.per_rule.entry(k).or_insert(0) += v;
                }
            }
        }

        let mut root_out =
            self.inner
                .run_pass_root(lang, ast, root, pass, out, child_remap)?;

        root_out.trace.rewrites.extend(trace.rewrites);
        root_out.stats.visited += stats.visited;
        root_out.stats.rewritten += stats.rewritten;
        for (k, v) in stats.per_rule {
            *root_out.stats.per_rule.entry(k).or_insert(0) += v;
        }
        root_out.stats.duration = started.elapsed();
        Ok(root_out)
    }

    /// Run an ordered pipeline of passes, feeding each pass's output into the
    /// next.
    pub fn run_pipeline(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        passes: &[&str],
    ) -> Result<TransformOutput, LangError> {
        let started = Instant::now();
        let mut cur_ast = ast.clone();
        let mut cur_root = root;
        let mut trace = RewriteTrace::new();
        let mut stats = PassStats::new();

        for pass in passes {
            let out = self.run_pass(lang, &cur_ast, cur_root, pass)?;
            cur_ast = out.ast;
            cur_root = out.root;
            trace.rewrites.extend(out.trace.rewrites);
            stats.visited += out.stats.visited;
            stats.rewritten += out.stats.rewritten;
            for (k, v) in out.stats.per_rule {
                *stats.per_rule.entry(k).or_insert(0) += v;
            }
        }

        stats.duration = started.elapsed();
        Ok(TransformOutput::new(cur_ast, cur_root, trace, stats))
    }
}

impl Default for ParallelEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TransformEngine for ParallelEngine {
    fn transform(&self, lang: &Language, req: &TransformRequest) -> Result<TransformOutput, LangError> {
        match &req.mode {
            TransformMode::Pass => self.run_pass(lang, &req.ast, req.root, req.pass()),
            TransformMode::Pipeline(passes) => {
                let names: Vec<&str> = passes.iter().map(|s| s.as_str()).collect();
                self.run_pipeline(lang, &req.ast, req.root, &names)
            }
            TransformMode::ApplyAt { .. } | TransformMode::Plan => self.inner.transform(lang, req),
        }
    }

    fn find_matches(
        &self,
        lang: &Language,
        req: &TransformRequest,
    ) -> Result<Vec<Match>, LangError> {
        Ok(self
            .inner
            .find_matches(lang, &req.ast, req.root, req.pass()))
    }

    fn box_clone(&self) -> Box<dyn TransformEngine> {
        Box::new(self.clone())
    }

    fn with_natives(&self, natives: Arc<dyn NativeRewrite>) -> Box<dyn TransformEngine> {
        let mut p = self.clone();
        p.inner = p.inner.with_native_registry(natives);
        Box::new(p)
    }
}

/// The child indices referenced by `root`'s `children` payload, if any.
fn child_refs(ast: &Ast, root: AstRef) -> Vec<usize> {
    let Some(node) = ast.get(root) else {
        return Vec::new();
    };
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|u| u as usize))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract the subtree rooted at `root` into a fresh arena, returning it along
/// with a `new_index -> old_index` map.
///
/// Children are remapped into the new arena and shared descendants are
/// deduplicated, mirroring [`Ast::subtree`].
fn extract_subtree(ast: &Ast, root: AstRef) -> Result<(Ast, Vec<usize>), LangError> {
    let mut out = Ast::new();
    let mut old_to_new: HashMap<usize, usize> = HashMap::new();
    let mut new_to_old: Vec<usize> = Vec::new();
    subtree_into(ast, root.index(), &mut old_to_new, &mut new_to_old, &mut out)?;
    Ok((out, new_to_old))
}

fn subtree_into(
    ast: &Ast,
    idx: usize,
    old_to_new: &mut HashMap<usize, usize>,
    new_to_old: &mut Vec<usize>,
    out: &mut Ast,
) -> Result<usize, LangError> {
    if let Some(&n) = old_to_new.get(&idx) {
        return Ok(n);
    }

    let node = ast.at(AstRef::new(idx))?;
    let mut child_new_indices: Vec<u64> = Vec::new();
    if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
        for v in children {
            if let Some(c) = v.as_u64() {
                child_new_indices.push(subtree_into(ast, c as usize, old_to_new, new_to_old, out)? as u64);
            }
        }
    }

    let mut new_payload = node.payload.clone();
    if let Some(children) = new_payload.get_mut("children").and_then(|c| c.as_array_mut()) {
        for (i, new_idx) in child_new_indices.iter().enumerate() {
            if let Some(slot) = children.get_mut(i) {
                *slot = Value::from(*new_idx);
            }
        }
    }

    let new_idx = out.push(AstNode {
        kind: node.kind.clone(),
        span: node.span,
        payload: new_payload,
    }).index();
    new_to_old.push(idx);
    old_to_new.insert(idx, new_idx);
    Ok(new_idx)
}

/// Add `by` to every arena-index reference in `ast`.
///
/// The `children` array and any payload field whose name matches a declared
/// child role in `lang.ast` are treated as node references. This keeps the
/// merge data-driven and does not hardcode kind-specific field names.
fn offset_ast(ast: &mut Ast, by: usize, lang: &Language) {
    for i in 0..ast.len() {
        let Some(node) = ast.get_mut(AstRef::new(i)) else {
            continue;
        };
        offset_value_children(&mut node.payload, "children", by);
        if let Some(shape) = lang.ast.get_by_identity(&node.kind) {
            if let Some(roles) = shape.children.as_ref() {
                for ChildRoleSpec { role, .. } in roles {
                    offset_value_children(&mut node.payload, role, by);
                }
            }
        }
    }
}

fn offset_value_children(payload: &mut Value, key: &str, by: usize) {
    let Some(v) = payload.get_mut(key) else {
        return;
    };
    if let Some(n) = v.as_u64() {
        *v = Value::from(n as usize + by);
        return;
    }
    if let Some(arr) = v.as_array_mut() {
        for c in arr {
            if let Some(n) = c.as_u64() {
                *c = Value::from(n as usize + by);
            }
        }
    }
}
