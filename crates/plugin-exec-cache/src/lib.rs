//! # handl-plugin-exec-cache
//!
//! `ExecControl` rule that memoises evaluation results.
//!
//! Ported from `CallCache` / `CacheHost` in
//! `plugin-vm-common/src/capabilities/cache/`. The `before_eval` hook hashes
//! the AST (kind + canonical payload) and, on a cache hit, returns
//! [`ControlFlow::Skip(value)`] so `BasicEvaluator` uses the cached result
//! without dispatching to a behavior rule. The `after_eval` hook stores the
//! freshly computed result in the cache.
//!
//! The cache is keyed by the serialised `(kind, payload)` pair and shared
//! via an `Arc<Mutex<...>>` so it persists across recursive dispatches
//! within a single evaluation.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use handl_platform::{
    AstNode, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, ControlFlow, EvalContext, ExecControl, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

/// A cache key derived from an AST's kind and canonical payload.
///
/// Two AST nodes with the same kind and the same serialised payload produce
/// the same key, enabling cache hits across dispatches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AstKey {
    /// The `AstNode.kind`.
    pub kind: String,
    /// The canonical JSON serialisation of `AstNode.payload`.
    pub payload_json: String,
}

impl AstKey {
    /// Build a cache key from an [`AstNode`].
    pub fn from_ast(ast: &AstNode) -> Self {
        let payload_json = serde_json::to_string(&ast.payload).unwrap_or_else(|_| "null".into());
        AstKey {
            kind: ast.kind.clone(),
            payload_json,
        }
    }
}

/// Exec-control rule that caches evaluation results by AST identity.
///
/// `before_eval` looks up the AST key; on a hit it returns
/// [`ControlFlow::Skip`] with the cached value, short-circuiting dispatch.
/// `after_eval` stores the result for future hits.
///
/// The cache is shared via an `Arc<Mutex<HashMap<...>>>` so a single
/// `CacheControl` instance covers an entire recursive evaluation.
#[derive(Clone, Default)]
pub struct CacheControl {
    entries: Arc<Mutex<HashMap<AstKey, Value>>>,
}

impl CacheControl {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Looks up a cached result for the given AST.
    pub fn get(&self, ast: &AstNode) -> Option<Value> {
        let key = AstKey::from_ast(ast);
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
    }

    /// Clears all cached entries.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.clear();
        }
    }
}

impl std::fmt::Debug for CacheControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheControl")
            .field("len", &self.len())
            .finish()
    }
}

impl ExecControl for CacheControl {
    fn before_eval(
        &self,
        ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        let key = AstKey::from_ast(ast);
        if let Ok(guard) = self.entries.lock() {
            if let Some(cached) = guard.get(&key) {
                return Ok(ControlFlow::Skip(cached.clone()));
            }
        }
        Ok(ControlFlow::Continue)
    }

    fn after_eval(
        &self,
        ast: &AstNode,
        result: &Value,
        _ctx: &mut EvalContext,
    ) -> Result<(), PlatformError> {
        let key = AstKey::from_ast(ast);
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(key, result.clone());
        }
        Ok(())
    }
}

/// Plugin that contributes [`CacheControl`] to the [`BasicEvaluator`].
pub struct ExecCachePlugin {
    cache: CacheControl,
    registered: AtomicBool,
}

impl ExecCachePlugin {
    /// Create a new `ExecCachePlugin`. The shared [`CacheControl`] handle
    /// can be retrieved via [`cache`](Self::cache) after contribution.
    pub fn new() -> Self {
        Self {
            cache: CacheControl::new(),
            registered: AtomicBool::new(false),
        }
    }

    /// Returns a handle to the shared cache.
    pub fn cache(&self) -> CacheControl {
        self.cache.clone()
    }
}

impl Default for ExecCachePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExecCachePlugin {
    fn id(&self) -> &str {
        "exec.cache"
    }

    fn name(&self) -> &str {
        "exec-cache"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if !self.registered.load(Ordering::SeqCst) {
            self.registered.store(true, Ordering::SeqCst);
            ctx.contribute_exec_control(Arc::new(self.cache.clone()) as Arc<dyn ExecControl>)?;
        }
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        BasicEvaluator, Behavior, CapabilityId, CapabilityRequest, Eval, EvalBehavior,
        EvalRequest, KitRegistry, PluginContext, PluginRegistry, RuleAcceptor,
    };
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A "compute" rule that returns the payload's "value" field but
    /// increments a shared counter so we can detect whether it actually ran.
    struct Compute {
        calls: Arc<AtomicUsize>,
    }
    impl Behavior<Eval> for Compute {
        fn kind(&self) -> &str {
            "compute"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            req.result = Some(req.ast.payload.get("value").cloned().unwrap_or(Value::Null));
            Ok(())
        }
    }
    impl EvalBehavior for Compute {}

    fn run_eval(evaluator: &BasicEvaluator, payload: Value) -> Result<Value, PlatformError> {
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = evaluator.run(
            &CapabilityRequest {
                capability: CapabilityId::from("eval"),
                payload,
            },
            &mut pctx,
        )?;
        Ok(resp.0)
    }

    fn compute(value: i64) -> Value {
        json!({ "kind": "compute", "value": value })
    }

    #[test]
    fn second_identical_call_returns_cached_result_without_dispatch() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(Compute { calls: calls.clone() }) as Arc<dyn EvalBehavior>)
            .unwrap();
        let cache = CacheControl::new();
        evaluator.add_exec_control(Arc::new(cache.clone()));

        // First call: cache miss, dispatch runs.
        let r1 = run_eval(&evaluator, compute(42)).unwrap();
        assert_eq!(r1, json!(42));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // Second identical call: cache hit, dispatch skipped.
        let r2 = run_eval(&evaluator, compute(42)).unwrap();
        assert_eq!(r2, json!(42));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "cached call must not re-dispatch"
        );
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn distinct_payloads_are_not_cached_hits() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(Compute { calls: calls.clone() }) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator.add_exec_control(Arc::new(CacheControl::new()));

        let r1 = run_eval(&evaluator, compute(1)).unwrap();
        let r2 = run_eval(&evaluator, compute(2)).unwrap();
        assert_eq!(r1, json!(1));
        assert_eq!(r2, json!(2));
        assert_eq!(calls.load(Ordering::SeqCst), 2, "distinct payloads both dispatch");
    }
}
