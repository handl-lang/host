//! # handl-plugin-engine-transform-rewrite
//!
//! The first [`TransformEngine`] implementation (ADR-0094): a
//! rule-interpreting rewriting engine that generalizes the legacy
//! `handl-runtime::desugar_pass` machinery.
//!
//! - [`RewriteEngine::run_pass`] walks a pass's [`TransformDef`]s
//!   bottom-up over the `Ast` arena with a `remap` old→new index table —
//!   the proven `apply_desugar` algorithm, generalized to any named pass.
//! - Dispatch is data-driven: a `kind → Vec<TransformDef>` table built once
//!   per pass from the engine's [`RuleSource`]; no `kind ==` checks exist in
//!   the engine core.
//! - `TransformRule::Native(name)` dispatches through the wired
//!   [`NativeRewrite`] registry. [`with_desugar_natives`](RewriteEngine::with_desugar_natives)
//!   wires the remaining semantic desugar handlers (`data`,
//!   `match`→recursor) as named natives under `desugar.*`, consulted as a
//!   **fallback** for kinds with no declarative rule — the fixed-shape
//!   forms (`while`, `do-while`, `for`, `nullish`, `opt-chain`) are
//!   declarative `TransformDef`s in `data/transforms.yaml`.
//! - Every run returns a [`TransformOutput`] with a [`RewriteTrace`]
//!   (input→output provenance) and [`PassStats`] (visited/rewritten/
//!   per-rule counts + duration).

#![warn(missing_docs)]

/// [`TransformCondition`](handl_language::TransformCondition) evaluation —
/// shared by this engine and by other consumers of the predicate vocabulary
/// (e.g. `plugin-engine-lint` `Match` lint rules, ADR-0097).
pub mod condition;
mod dispatch;
pub mod native;
mod step;
mod walk;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use handl_language::{Ast, AstRef, LangError, Language};
use handl_plugin_engine_transform::native::{NativeRewrite, RewriteCtx};
use handl_plugin_engine_transform::output::{Rewrite, RewriteTrace, TransformOutput};
use handl_plugin_engine_transform::request::{TransformMode, TransformRequest};
use handl_plugin_engine_transform::source::{LanguageRuleSource, RuleSource};
use handl_plugin_engine_transform::{Match, TransformEngine};

use crate::condition::matches_condition;
use crate::dispatch::Dispatch;
use crate::walk::PassRun;

/// Monotonic id of the pass being run.
///
/// Bumped once per built-in pass so natives can keep per-pass caches
/// (e.g. the desugar constructor index) keyed by an epoch that never
/// repeats while an input arena is alive.
pub static PASS_EPOCH: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// The current pass epoch (see [`PASS_EPOCH`]).
pub fn pass_epoch() -> u64 {
    PASS_EPOCH.load(std::sync::atomic::Ordering::Relaxed)
}

/// Hard node-count budget for [`Traversal::Fixpoint`] iterations — a
/// growing rewrite that never converges is cut off here rather than
/// exhausting memory. Override with
/// [`RewriteEngine::with_node_budget`].
pub use crate::walk::MAX_NODES;

/// The traversal strategy of a pass run (ADR-0094 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Traversal {
    /// Insertion-order walk (children before parents in the Pratt arena
    /// layout) — the proven `apply_desugar` algorithm. This is the default.
    #[default]
    BottomUp,
    /// Recursive pre-order descent from the root: a rule fires on a node
    /// before its kept children are visited. Native rules and pass-through
    /// nodes process children first so the `remap` table is populated.
    TopDown,
    /// Repeat [`BottomUp`](Traversal::BottomUp) until an iteration performs
    /// no rewrite, or `max` iterations is reached. A hard node-count budget
    /// (`walk::MAX_NODES`) bounds non-converging rewrites.
    Fixpoint {
        /// Maximum number of bottom-up iterations.
        max: usize,
    },
}

/// The rule-interpreting transform engine (ADR-0094 §4).
///
/// Cheap to clone (all shared state is behind `Arc`); `Clone` backs
/// [`TransformEngine::box_clone`].
#[derive(Clone)]
pub struct RewriteEngine {
    /// Where rules come from (default: `lang.transforms`).
    rule_source: Arc<dyn RuleSource>,
    /// The wired native-rewrite registry, if any.
    natives: Option<Arc<dyn NativeRewrite>>,
    /// `(pass, node_kind) → native name` fallback consulted when a kind has
    /// no declarative rule — populated by
    /// [`with_desugar_natives`](Self::with_desugar_natives).
    native_fallback: HashMap<(String, String), String>,
    /// The traversal used by [`run_pass`](Self::run_pass).
    traversal: Traversal,
    /// Hard node-count budget for [`Fixpoint`](Traversal::Fixpoint)
    /// iterations. Defaults to [`MAX_NODES`]; configurable so callers
    /// (and tests) can bound non-converging rewrites more tightly.
    node_budget: usize,
}

impl Default for RewriteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RewriteEngine {
    /// Create an engine reading rules from `lang.transforms`.
    pub fn new() -> Self {
        Self {
            rule_source: Arc::new(LanguageRuleSource::new()),
            natives: None,
            native_fallback: HashMap::new(),
            traversal: Traversal::BottomUp,
            node_budget: MAX_NODES,
        }
    }

    /// Create an engine reading rules from a custom [`RuleSource`]
    /// (codemod files, extra fragment sets, …).
    pub fn with_rule_source(src: Box<dyn RuleSource>) -> Self {
        Self {
            rule_source: Arc::from(src),
            ..Self::new()
        }
    }

    /// Wire the desugar native registry (ADR-0094 §6).
    ///
    /// Registers the remaining `try_rust_desugar` handlers under their
    /// `desugar.<form>` names **and** installs the `(desugar, kind)` native
    /// fallback table, so the `"desugar"` pass rewrites `data` and `match`
    /// nodes that have no declarative rule — with declarative rules always
    /// taking precedence. (`while`, `do-while`, `for`, `nullish`, and
    /// `opt-chain` desugar declaratively via `transforms.yaml`.)
    pub fn with_desugar_natives(self) -> Self {
        // When a host installed the transform overlay (e.g.
        // `plugin-lib-handl::transform::install_desugar_overlay`), layer
        // it over the Rust desugar registry: `handl:desugar.*` names the
        // overlay provides go to the `.hnl` rewrites, everything else to
        // the Rust natives (which stay registered under the same names as
        // the bootstrap/fallback layer).
        let base: Arc<dyn NativeRewrite> = Arc::new(native::desugar_native_registry());
        let natives: Arc<dyn NativeRewrite> =
            match handl_plugin_engine_transform::native::transform_overlay() {
                Some(overlay) => Arc::new(
                    handl_plugin_engine_transform::native::ChainedRewrite::new(vec![
                        overlay, base,
                    ]),
                ),
                None => base,
            };
        let mut engine = self.with_native_registry(natives);
        for (kind, name) in native::DESUGAR_NATIVES {
            engine
                .native_fallback
                .insert(("desugar".to_string(), kind.to_string()), name.to_string());
        }
        engine
    }

    /// Wire the desugar native registry **and** layer a handl-native
    /// [`NativeRewrite`] on top of it (ADR-0094). `handl_natives`
    /// resolves `handl:<mod>.<fn>` rules against the kit's `.hnl` modules and
    /// falls back to the `desugar.*` registry for everything else; the
    /// `native_fallback` table is populated so kind-based dispatch still
    /// works. This is the single production call that makes both `desugar.*`
    /// and `handl:` transform rules resolve in one engine.
    pub fn with_desugar_and_handl_natives(self, handl_natives: Arc<dyn NativeRewrite>) -> Self {
        let mut engine = self.with_native_registry(handl_natives);
        for (kind, name) in native::DESUGAR_NATIVES {
            engine
                .native_fallback
                .insert(("desugar".to_string(), kind.to_string()), name.to_string());
        }
        engine
    }

    /// Wire a custom [`NativeRewrite`] registry (no fallback table — custom
    /// natives are reachable only through explicit
    /// `TransformRule::Native(name)` rules).
    pub fn with_native_registry(mut self, natives: Arc<dyn NativeRewrite>) -> Self {
        self.natives = Some(natives);
        self
    }

    /// Set the traversal used by [`run_pass`](Self::run_pass) and
    /// [`run_pipeline`](Self::run_pipeline).
    pub fn with_traversal(mut self, traversal: Traversal) -> Self {
        self.traversal = traversal;
        self
    }

    /// Set the hard node-count budget for
    /// [`Fixpoint`](Traversal::Fixpoint) iterations (default:
    /// [`MAX_NODES`]). A pass whose output arena exceeds the budget
    /// fails with a [`LangError`] rather than exhausting memory.
    pub fn with_node_budget(mut self, node_budget: usize) -> Self {
        self.node_budget = node_budget;
        self
    }

    /// Run one named pass over `ast` using the engine's configured
    /// [`Traversal`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] when a rule fails, a matched `Native` rule has
    /// no wired registry, or a [`Fixpoint`](Traversal::Fixpoint) pass exceeds
    /// its iteration/node budget.
    pub fn run_pass(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        pass: &str,
    ) -> Result<TransformOutput, LangError> {
        self.run_pass_traversal(lang, ast, root, pass, self.traversal)
    }

    /// Run one named pass with an explicit [`Traversal`].
    ///
    /// # Errors
    ///
    /// See [`run_pass`](Self::run_pass).
    pub fn run_pass_traversal(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        pass: &str,
        traversal: Traversal,
    ) -> Result<TransformOutput, LangError> {
        match traversal {
            Traversal::BottomUp => self.single_pass(lang, ast, root, pass, false),
            Traversal::TopDown => self.single_pass(lang, ast, root, pass, true),
            Traversal::Fixpoint { max } => self.run_fixpoint(lang, ast, root, pass, max),
        }
    }

    /// Run an ordered pipeline of passes, feeding each pass's output arena
    /// into the next.
    ///
    /// The returned [`TransformOutput`] carries the final arena and root;
    /// the trace concatenates every pass's rewrites and the stats aggregate
    /// visits/rewrites across the pipeline (per-rule counts are merged).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] when any pass fails.
    pub fn run_pipeline(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        passes: &[&str],
    ) -> Result<TransformOutput, LangError> {
        let mut cur_ast = ast.clone();
        let mut cur_root = root;
        let mut trace = RewriteTrace::new();
        let mut stats = handl_plugin_engine_transform::output::PassStats::new();
        for pass in passes {
            let out = self.run_pass(lang, &cur_ast, cur_root, pass)?;
            cur_ast = out.ast;
            cur_root = out.root;
            trace.rewrites.extend(out.trace.rewrites);
            stats.visited += out.stats.visited;
            stats.rewritten += out.stats.rewritten;
            stats.duration += out.stats.duration;
            for (k, v) in out.stats.per_rule {
                *stats.per_rule.entry(k).or_insert(0) += v;
            }
        }
        Ok(TransformOutput::new(cur_ast, cur_root, trace, stats))
    }

    /// Apply a single rule to a single node (surgical rewrite — LSP/codemod).
    ///
    /// The rest of the tree passes through unchanged. The rule's `when`
    /// condition must hold at `at` (a mismatch is an error, not a silent
    /// pass-through).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] when `rule_id` is unknown, `at` is out of
    /// bounds, or the rewrite fails.
    pub fn apply_rule(
        &self,
        lang: &Language,
        ast: &Ast,
        at: AstRef,
        rule_id: &str,
    ) -> Result<TransformOutput, LangError> {
        let def = lang
            .transforms
            .get_by_id(rule_id)
            .ok_or_else(|| LangError::engine(format!("unknown transform rule '{rule_id}'")))?;
        if at.index() >= ast.len() {
            return Err(LangError::engine(format!(
                "ApplyAt target {} out of bounds (arena len {})",
                at.index(),
                ast.len()
            )));
        }
        // Build the dispatch for the rule's own pass so `Apply` steps can
        // resolve sibling rule ids.
        let dispatch = Dispatch::build(&*self.rule_source, lang, &def.pass);
        let def = dispatch
            .rule_by_id(rule_id)
            .ok_or_else(|| LangError::engine(format!("unknown transform rule '{rule_id}'")))?;
        let mut run = PassRun::new(
            lang,
            ast,
            def.pass.as_str(),
            &dispatch,
            self.natives.clone(),
            &self.native_fallback,
            false,
            Some((at.index(), def)),
        );
        let mut out = Ast::new();
        run.run_bottom_up(&mut out)?;
        let root = run.output_root(AstRef::new(ast.len() - 1), &out);
        let (trace, stats) = (run.trace.clone(), run.stats.clone());
        Ok(TransformOutput::new(out, root, trace, stats))
    }

    /// Match-only: find every site in the subtree rooted at `root` where a
    /// rule of `pass` would fire, without rewriting.
    ///
    /// `Match.bindings` is currently empty (positions are resolvable from
    /// `Match.at` via the shape roles).
    pub fn find_matches(&self, lang: &Language, ast: &Ast, root: AstRef, pass: &str) -> Vec<Match> {
        let dispatch = Dispatch::build(&*self.rule_source, lang, pass);
        let reachable = reachable_from(ast, root);
        let mut matches = Vec::new();
        for i in reachable {
            let Some(node) = ast.get(AstRef::new(i)) else {
                continue;
            };
            let rules = dispatch.rules_for(&node.kind);
            if let Some(rules) = rules {
                for def in rules {
                    let fires = match &def.when {
                        Some(cond) => matches_condition(lang, ast, node, cond),
                        None => true,
                    };
                    if fires {
                        matches.push(Match {
                            at: AstRef::new(i),
                            rule_id: def.id.clone(),
                            bindings: Default::default(),
                        });
                        break; // first match wins, same as firing order
                    }
                }
            } else if let Some(name) = self
                .native_fallback
                .get(&(pass.to_string(), node.kind.clone()))
            {
                // Native fallback (ADR-0094 §6): kinds with no declarative
                // rule for this pass may still rewrite via a wired native —
                // a plan should show those sites too. (Only when NO
                // declarative rule exists; a `when` miss is a pass-through,
                // matching `PassRun::process`.) When a registry is wired,
                // fire the native on a scratch arena: a decline is not a
                // match site.
                let fires = match &self.natives {
                    Some(natives) => {
                        let mut scratch = Ast::new();
                        let remap: Vec<Option<usize>> = (0..ast.len()).map(Some).collect();
                        let mut ctx =
                            RewriteCtx::new(lang, node, AstRef::new(i), ast, &mut scratch, &remap);
                        natives
                            .rewrite(name, &mut ctx)
                            .map(|_| !ctx.declined())
                            .unwrap_or(false)
                    }
                    None => true,
                };
                if fires {
                    matches.push(Match {
                        at: AstRef::new(i),
                        rule_id: format!("native:{name}"),
                        bindings: Default::default(),
                    });
                }
            }
        }
        matches
    }

    /// One single-arena pass.
    fn single_pass(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        pass: &str,
        topdown: bool,
    ) -> Result<TransformOutput, LangError> {
        PASS_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let started = Instant::now();
        let dispatch = Dispatch::build(&*self.rule_source, lang, pass);
        let mut run = PassRun::new(
            lang,
            ast,
            pass,
            &dispatch,
            self.natives.clone(),
            &self.native_fallback,
            topdown,
            None,
        );
        let mut out = Ast::new();
        if topdown {
            run.run_top_down(&mut out, root)?;
        } else {
            run.run_bottom_up(&mut out)?;
        }
        run.stats.duration = started.elapsed();
        let root = run.output_root(root, &out);
        let (trace, stats) = (run.trace.clone(), run.stats.clone());
        Ok(TransformOutput::new(out, root, trace, stats))
    }

    /// `Fixpoint { max }`: repeat bottom-up passes until stable or capped.
    fn run_fixpoint(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        pass: &str,
        max: usize,
    ) -> Result<TransformOutput, LangError> {
        let started = Instant::now();
        let mut cur_ast = ast.clone();
        let mut cur_root = root;
        let mut trace = RewriteTrace::new();
        let mut stats = handl_plugin_engine_transform::output::PassStats::new();
        for _iteration in 0..max {
            if cur_ast.len() > self.node_budget {
                return Err(LangError::engine(format!(
                    "fixpoint pass '{pass}' exceeded the node budget ({}) after {} node(s)",
                    self.node_budget,
                    cur_ast.len()
                )));
            }
            let out = self.single_pass(lang, &cur_ast, cur_root, pass, false)?;
            stats.visited += out.stats.visited;
            stats.rewritten += out.stats.rewritten;
            for (k, v) in out.stats.per_rule {
                *stats.per_rule.entry(k).or_insert(0) += v;
            }
            trace.rewrites.extend(out.trace.rewrites);
            let done = out.stats.rewritten == 0;
            cur_ast = out.ast;
            cur_root = out.root;
            if done {
                stats.duration = started.elapsed();
                return Ok(TransformOutput::new(cur_ast, cur_root, trace, stats));
            }
        }
        stats.duration = started.elapsed();
        // Stable-or-budget: `max` iterations reached without convergence —
        // return the last arena (not an error; the bound was honored).
        Ok(TransformOutput::new(cur_ast, cur_root, trace, stats))
    }

    /// Process only the root node after its children have already been
    /// rewritten and placed in `out`.
    ///
    /// `child_remap` must map the old child indices referenced by `root`
    /// (in `ast`) to the new indices already present in `out`. The new root
    /// node — and any nodes created by the root rule — are appended to `out`.
    /// `fresh_base` seeds the pass's [`RewriteStep::Fresh`] counter so a
    /// root-level rewrite mints `__fresh_*` names after every name the
    /// already-rewritten children minted (matching sequential numbering).
    ///
    /// This is the sequential tail used by the parallel engine: children are
    /// rewritten in parallel, then the root is processed once.
    pub fn run_pass_root(
        &self,
        lang: &Language,
        ast: &Ast,
        root: AstRef,
        pass: &str,
        mut out: Ast,
        child_remap: Vec<Option<usize>>,
        fresh_base: usize,
    ) -> Result<TransformOutput, LangError> {
        let started = Instant::now();
        if child_remap.len() < ast.len() {
            return Err(LangError::engine(
                "child_remap is shorter than the input arena".to_string(),
            ));
        }
        let dispatch = Dispatch::build(&*self.rule_source, lang, pass);
        let mut run = PassRun::new(
            lang,
            ast,
            pass,
            &dispatch,
            self.natives.clone(),
            &self.native_fallback,
            false,
            None,
        );
        run.set_remap(child_remap);
        run.fresh_id = fresh_base;
        let node = ast.at(root)?;
        run.process(root.index(), node, &mut out)?;
        run.stats.duration = started.elapsed();
        let root = run.output_root(root, &out);
        let (trace, stats) = (run.trace.clone(), run.stats.clone());
        Ok(TransformOutput::new(out, root, trace, stats))
    }
}

impl TransformEngine for RewriteEngine {
    fn transform(
        &self,
        lang: &Language,
        req: &TransformRequest,
    ) -> Result<TransformOutput, LangError> {
        match &req.mode {
            TransformMode::Pass => self.run_pass(lang, &req.ast, req.root, req.pass()),
            TransformMode::Pipeline(passes) => {
                let names: Vec<&str> = passes.iter().map(|s| s.as_str()).collect();
                self.run_pipeline(lang, &req.ast, req.root, &names)
            }
            TransformMode::ApplyAt { at, rule_id } => self.apply_rule(lang, &req.ast, *at, rule_id),
            TransformMode::Plan => {
                // Matched sites + proposed rewrites, no new arena: the
                // output is the input unchanged; each match is recorded as
                // a `Rewrite { from == to }` proposal.
                let matches = self.find_matches(lang, &req.ast, req.root, req.pass());
                let mut stats = handl_plugin_engine_transform::output::PassStats::new();
                stats.visited = req.ast.len();
                let rewrites = matches
                    .into_iter()
                    .map(|m| Rewrite {
                        from: m.at,
                        to: m.at,
                        rule_id: m.rule_id,
                    })
                    .collect();
                Ok(TransformOutput::new(
                    req.ast.clone(),
                    req.root,
                    RewriteTrace { rewrites },
                    stats,
                ))
            }
        }
    }

    fn find_matches(
        &self,
        lang: &Language,
        req: &TransformRequest,
    ) -> Result<Vec<Match>, LangError> {
        Ok(self.find_matches(lang, &req.ast, req.root, req.pass()))
    }

    fn box_clone(&self) -> Box<dyn TransformEngine> {
        Box::new(self.clone())
    }

    fn with_natives(&self, natives: Arc<dyn NativeRewrite>) -> Box<dyn TransformEngine> {
        let mut engine = self.clone();
        engine.natives = Some(natives);
        Box::new(engine)
    }
}

/// Collect the input-arena indices reachable from `root` through `children`
/// arrays, in ascending index order.
fn reachable_from(ast: &Ast, root: AstRef) -> Vec<usize> {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![root.index()];
    while let Some(idx) = stack.pop() {
        if !seen.insert(idx) {
            continue;
        }
        if let Some(node) = ast.get(AstRef::new(idx)) {
            if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
                for v in children {
                    if let Some(c) = v.as_u64() {
                        stack.push(c as usize);
                    }
                }
            }
        }
    }
    seen.into_iter().collect()
}
