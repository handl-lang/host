#![cfg(feature = "e2e")]

use handl_lexer_canonical::CanonicalLexer;
use handl_lib_concurrent::ConcurrencyModule;
use handl_plugin_parser::Parser as _;
use handl_parser_canonical::CanonicalParser;
use handl_vm_backend_rust_interpreter::{CanonicalEval, CanonicalVm};
use handl_vm_common::{Machine, ModuleHost};
use handl_vm_common::prelude::{
    AsyncFunction, Expr, Function, GenericFunction, Host, LazyFunction, Operation,
};
use handl_vm_host_eval::EvalHost;
use handl_vm_values_json::NonFunctionValue;
use serde_json::json;

#[derive(Clone)]
struct CombinedHost {
    eval: EvalHost,
    modules: ModuleHost,
}

impl CombinedHost {
    fn new() -> Self {
        Self {
            eval: EvalHost::default(),
            modules: ModuleHost::new().with(ConcurrencyModule::new()),
        }
    }
}

impl Host<NonFunctionValue> for CombinedHost {
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        self.eval.function(name).or_else(|| self.modules.function(name))
    }

    fn async_function(&self, name: &str) -> Option<&dyn AsyncFunction<NonFunctionValue>> {
        self.eval
            .async_function(name)
            .or_else(|| self.modules.async_function(name))
    }

    fn operation(&self, name: &str) -> Option<&dyn Operation<NonFunctionValue>> {
        self.eval
            .operation(name)
            .or_else(|| self.modules.operation(name))
    }

    fn lazy_function(&self, name: &str) -> Option<&dyn LazyFunction<NonFunctionValue>> {
        self.eval
            .lazy_function(name)
            .or_else(|| self.modules.lazy_function(name))
    }

    fn generic_function(&self, name: &str) -> Option<&dyn GenericFunction<NonFunctionValue>> {
        self.eval
            .generic_function(name)
            .or_else(|| self.modules.generic_function(name))
    }

    fn is_reserved(&self, name: &str) -> bool {
        self.eval.is_reserved(name) || self.modules.is_reserved(name)
    }
}

fn parse(source: &str) -> Expr<NonFunctionValue> {
    CanonicalParser
        .parse_text(source, &CanonicalLexer::default())
        .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e:?}"))
}

fn make_machine(host: &CombinedHost) -> Machine<CanonicalVm<CombinedHost>> {
    Machine::new(CanonicalVm::new(host.clone())).with_context(|ctx| {
        ctx.with_eval(CanonicalEval::new(host.clone(), None, None))
    })
}

fn assert_number(v: &NonFunctionValue, n: f64) {
    assert_eq!(v.as_number(), Some(n), "expected {n}, got {:?}", v.inner());
}

#[test]
fn spawn_returns_task() {
    let v = make_machine(&CombinedHost::new()).eval(&parse("spawn(() -> 1)")).unwrap();
    assert_eq!(v.get("type"), Some(&json!("task")));
}

#[test]
fn await_spawn_returns_one() {
    let v = make_machine(&CombinedHost::new())
        .eval(&parse("await(spawn(() -> 1))"))
        .unwrap();
    assert_number(&v, 1.0);
}

#[test]
fn channel_send_recv() {
    let host = CombinedHost::new();
    let ch = make_machine(&host)
        .eval(&parse("channel(null)"))
        .unwrap();

    let machine = Machine::new(CanonicalVm::new(host.clone())).with_context(|ctx| {
        ctx.with_namespace("ch", ch)
            .with_eval(CanonicalEval::new(host.clone(), None, None))
    });

    let _ = machine.eval(&parse("send(ch, 7)")).unwrap();
    let v = machine.eval(&parse("recv(ch)")).unwrap();
    assert_number(&v, 7.0);
}

#[test]
fn mutex_lock_returns_value() {
    let v = make_machine(&CombinedHost::new())
        .eval(&parse("apply((m) -> lock(m, () -> 42), mutex())"))
        .unwrap();
    assert_number(&v, 42.0);
}

#[test]
fn concurrent_block_sugar_spawns() {
    let v = make_machine(&CombinedHost::new())
        .eval(&parse("concurrent { () -> 1 }"))
        .unwrap();
    assert_eq!(v.get("type"), Some(&json!("task")));
}
