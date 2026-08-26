use handl_lib_concurrent::ConcurrencyModule;
use handl_vm_common::Module;
use handl_vm_values_json::{CanonicalContext, Eval, FunctionValue, NonFunctionValue};
use proptest::prelude::*;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn thunk(body: &str) -> NonFunctionValue {
    NonFunctionValue::new(FunctionValue {
        params: Vec::new(),
        return_schema: None,
        body: body.to_string(),
        patterns: Vec::new(),
    }.to_json())
}

#[derive(Clone)]
struct NumberEval;

impl Eval<NonFunctionValue> for NumberEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, handl_vm_values_json::Error> {
        let n: u64 = expr
            .parse()
            .map_err(|_| handl_vm_values_json::Error::PrimitiveError(format!("expected a u64, got {expr:?}")))?;
        Ok(NonFunctionValue::new(json!(n)))
    }
}

#[derive(Clone)]
struct PushToListEval {
    list: Arc<Mutex<Vec<u64>>>,
}

impl Eval<NonFunctionValue> for PushToListEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, handl_vm_values_json::Error> {
        let n: u64 = expr
            .parse()
            .map_err(|_| handl_vm_values_json::Error::PrimitiveError(format!("expected a u64, got {expr:?}")))?;
        self.list.lock().unwrap().push(n);
        Ok(NonFunctionValue::new(json!(n)))
    }
}

#[derive(Clone)]
struct LockingEval {
    module: Arc<ConcurrencyModule>,
    mutex: NonFunctionValue,
    list: Arc<Mutex<Vec<u64>>>,
}

impl Eval<NonFunctionValue> for LockingEval {
    fn eval_expr(
        &self,
        expr: &str,
        _vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, handl_vm_values_json::Error> {
        let inner = FunctionValue {
            params: Vec::new(),
            return_schema: None,
            body: expr.to_string(),
            patterns: Vec::new(),
        };
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(PushToListEval {
            list: self.list.clone(),
        }));
        self.module
            .function("lock")
            .unwrap()
            .call(&[self.mutex.clone(), NonFunctionValue::new(inner.to_json())], &mut ctx)
    }
}

proptest! {
    #[test]
    fn prop_spawn_await_roundtrip(v in any::<u64>()) {
        let module = ConcurrencyModule::default();
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(NumberEval));

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

        prop_assert_eq!(result, NonFunctionValue::new(json!(v)));
    }

    #[test]
    fn prop_channel_send_recv_roundtrip(s in any::<String>()) {
        let module = ConcurrencyModule::default();
        let mut ctx = CanonicalContext::new();

        let ch = module
            .function("channel")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(null))], &mut ctx)
            .unwrap();
        let value = NonFunctionValue::new(json!(s.clone()));
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

        prop_assert_eq!(received, NonFunctionValue::new(json!(s)));
    }

    #[test]
    fn prop_mutex_lock_unordered(values in prop::collection::vec(any::<u64>(), 1..16usize)) {
        let module = Arc::new(ConcurrencyModule::default());
        let mut init_ctx = CanonicalContext::new();
        let m = module
            .function("mutex")
            .unwrap()
            .call(&[], &mut init_ctx)
            .unwrap();

        let list = Arc::new(Mutex::new(Vec::new()));
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(LockingEval {
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
        let mut ctx = CanonicalContext::new();

        let r = module
            .function("atomic")
            .unwrap()
            .call(&[NonFunctionValue::new(json!(old))], &mut ctx)
            .unwrap();
        let swapped = module
            .function("atomic_cas")
            .unwrap()
            .call(
                &[
                    r.clone(),
                    NonFunctionValue::new(json!(old)),
                    NonFunctionValue::new(json!(new)),
                ],
                &mut ctx,
            )
            .unwrap();
        prop_assert_eq!(swapped, NonFunctionValue::new(json!(true)));

        let loaded = module
            .function("atomic_read")
            .unwrap()
            .call(&[r], &mut ctx)
            .unwrap();
        prop_assert_eq!(loaded, NonFunctionValue::new(json!(new)));
    }
}
