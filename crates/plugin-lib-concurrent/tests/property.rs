use handl_plugin_lib_concurrent::function_value::FunctionValue;
use handl_plugin_lib_concurrent::ConcurrencyModule;
use handl_platform::{EvalContext, EvalError, ExprEvaluator, Module};
use proptest::prelude::*;
use serde_json::json;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::rc::Rc;

fn thunk(body: &str) -> Value {
    FunctionValue {
        params: Vec::new(),
        return_schema: None,
        body: body.to_string(),
        patterns: Vec::new(),
        captured: Vec::new(),
    }
    .to_json()
}

#[derive(Clone)]
struct NumberEval;

impl ExprEvaluator for NumberEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, Value)],
    ) -> Result<Value, EvalError> {
        let n: u64 = expr.parse().map_err(|_| {
            EvalError::Other(format!("expected a u64, got {expr:?}"))
        })?;
        Ok(json!(n))
    }
}

#[derive(Clone)]
struct PushToListEval {
    list: Arc<Mutex<Vec<u64>>>,
}

impl ExprEvaluator for PushToListEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, Value)],
    ) -> Result<Value, EvalError> {
        let n: u64 = expr.parse().map_err(|_| {
            EvalError::Other(format!("expected a u64, got {expr:?}"))
        })?;
        self.list.lock().unwrap().push(n);
        Ok(json!(n))
    }
}

#[derive(Clone)]
struct LockingEval {
    module: Arc<ConcurrencyModule>,
    mutex: Value,
    list: Arc<Mutex<Vec<u64>>>,
}

impl ExprEvaluator for LockingEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, Value)],
    ) -> Result<Value, EvalError> {
        let inner = FunctionValue {
            params: Vec::new(),
            return_schema: None,
            body: expr.to_string(),
            patterns: Vec::new(),
            captured: Vec::new(),
        };
        let mut ctx = EvalContext::new();
        ctx.set_expr_evaluator(Rc::new(PushToListEval {
            list: self.list.clone(),
        }));
        self.module.function("lock").unwrap().call(
            &[self.mutex.clone(), inner.to_json()],
            &mut ctx,
        )
    }
}

proptest! {
    #[test]
    fn prop_spawn_await_roundtrip(v in any::<u64>()) {
        let module = ConcurrencyModule::default();
        let mut ctx = EvalContext::new();
        ctx.set_expr_evaluator(Rc::new(NumberEval));

        let work = thunk(&v.to_string());
        let task = module
            .function("spawn")
            .unwrap()
            .call(&[work], &mut ctx)
            .unwrap();
        let result = module
            .function("await")
            .unwrap()
            .call(&[task], &mut ctx)
            .unwrap();

        prop_assert_eq!(result, json!(v));
    }

    #[test]
    fn prop_channel_send_recv_roundtrip(s in any::<String>()) {
        let module = ConcurrencyModule::default();
        let mut ctx = EvalContext::new();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[json!(null)], &mut ctx)
            .unwrap();
        let value = json!(s.clone());
        module
            .function("send")
            .unwrap()
            .call(&[ch.clone(), value], &mut ctx)
            .unwrap();
        let received = module
            .function("recv")
            .unwrap()
            .call(&[ch], &mut ctx)
            .unwrap();

        prop_assert_eq!(received, json!(s));
    }

    #[test]
    fn prop_mutex_lock_unordered(values in prop::collection::vec(any::<u64>(), 1..16usize)) {
        let module = Arc::new(ConcurrencyModule::default());
        let mut init_ctx = EvalContext::new();
        let m = module
            .function("mutex")
            .unwrap()
            .call(&[], &mut init_ctx)
            .unwrap();

        let list = Arc::new(Mutex::new(Vec::new()));
        let mut ctx = EvalContext::new();
        ctx.set_expr_evaluator(Rc::new(LockingEval {
            module: module.clone(),
            mutex: m,
            list: list.clone(),
        }));

        let mut tasks = Vec::new();
        for v in &values {
            let work = thunk(&v.to_string());
            let task = module
                .function("spawn")
                .unwrap()
                .call(&[work], &mut ctx)
                .unwrap();
            tasks.push(task);
        }

        for task in tasks {
            module
                .function("await")
                .unwrap()
                .call(&[task], &mut ctx)
                .unwrap();
        }

        let mut collected = list.lock().unwrap().clone();
        let mut expected = values;
        collected.sort_unstable();
        expected.sort_unstable();
        prop_assert_eq!(collected, expected);
    }

    #[test]
    fn prop_atomic_read_write_cas((old, new) in (any::<u64>(), any::<u64>())) {
        let module = ConcurrencyModule::default();
        let mut ctx = EvalContext::new();

        let r = module
            .function("atomic")
            .unwrap()
            .call(&[json!(old)], &mut ctx)
            .unwrap();
        let swapped = module
            .function("atomic_cas")
            .unwrap()
            .call(
                &[
                    r.clone(),
                    json!(old),
                    json!(new),
                ],
                &mut ctx,
            )
            .unwrap();
        prop_assert_eq!(swapped, json!(true));

        let loaded = module
            .function("atomic_read")
            .unwrap()
            .call(&[r], &mut ctx)
            .unwrap();
        prop_assert_eq!(loaded, json!(new));
    }
}
