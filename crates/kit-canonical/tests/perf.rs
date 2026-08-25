//! Wall-clock performance tests for the canonical expression kit.
//!
//! These are intentionally budgeted so the normal `cargo test` run stays green.
//! The 10k-pipe cases are ignored because the deeply nested `fn.pipe` chain
//! overflows the default debug thread stack before completing.

use std::time::{Duration, Instant};

use handl_ast_canonical::Expr;
use handl_expr::ModuleOutput;
use handl_kit_canonical::CanonicalKit;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

fn pipe_expr(n: usize) -> String {
    let mut s = "1".to_string();
    for _ in 0..n {
        s.push_str(" |> f");
    }
    s
}

fn pipe_module(n: usize) -> String {
    format!("f(x) -> x; {}", pipe_expr(n))
}

fn parsed_module(kit: &CanonicalKit, n: usize) -> ModuleOutput {
    kit.parse_module_recovered(&pipe_module(n))
}

#[test]
fn parse_long_pipe_100() {
    let kit = CanonicalKit::new();
    let input = pipe_expr(100);
    let start = Instant::now();
    let result: Result<Expr<NonFunctionValue>, PlatformError> = kit.parse(&input);
    let elapsed = start.elapsed();
    eprintln!("parse 100 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
    assert!(elapsed < Duration::from_millis(5), "parse 100 took {:?}", elapsed);
}

#[test]
fn parse_long_pipe_1000() {
    let kit = CanonicalKit::new();
    let input = pipe_expr(1000);
    let start = Instant::now();
    let result: Result<Expr<NonFunctionValue>, PlatformError> = kit.parse(&input);
    let elapsed = start.elapsed();
    eprintln!("parse 1000 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
    assert!(elapsed < Duration::from_millis(150), "parse 1000 took {:?}", elapsed);
}

#[test]
fn parse_long_pipe_10000() {
    let kit = CanonicalKit::new();
    let input = pipe_expr(10000);
    let start = Instant::now();
    let result: Result<Expr<NonFunctionValue>, PlatformError> = kit.parse(&input);
    let elapsed = start.elapsed();
    eprintln!("parse 10000 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
    assert!(elapsed < Duration::from_secs(1), "parse 10000 took {:?}", elapsed);
}

#[test]
fn typecheck_long_pipe_100() {
    let kit = CanonicalKit::new();
    let output = parsed_module(&kit, 100);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);

    let start = Instant::now();
    let result = kit.typecheck(&output.stmts);
    let elapsed = start.elapsed();
    eprintln!("typecheck 100 pipes: {:?}", elapsed);
    assert!(result.is_ok() && result.unwrap().success, "typecheck failed");
    assert!(elapsed < Duration::from_millis(10), "typecheck 100 took {:?}", elapsed);
}

#[test]
fn typecheck_long_pipe_1000() {
    let kit = CanonicalKit::new();
    let output = parsed_module(&kit, 1000);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);

    let start = Instant::now();
    let result = kit.typecheck(&output.stmts);
    let elapsed = start.elapsed();
    eprintln!("typecheck 1000 pipes: {:?}", elapsed);
    assert!(result.is_ok() && result.unwrap().success, "typecheck failed");
    assert!(elapsed < Duration::from_millis(100), "typecheck 1000 took {:?}", elapsed);
}

#[test]
#[ignore = "10k nested fn.pipe overflows the default stack in debug builds"]
fn typecheck_long_pipe_10000() {
    let kit = CanonicalKit::new();
    let output = parsed_module(&kit, 10000);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);

    let start = Instant::now();
    let result = kit.typecheck(&output.stmts);
    let elapsed = start.elapsed();
    eprintln!("typecheck 10000 pipes: {:?}", elapsed);
    assert!(result.is_ok() && result.unwrap().success, "typecheck failed");
    assert!(elapsed < Duration::from_secs(1), "typecheck 10000 took {:?}", elapsed);
}

#[test]
fn eval_long_pipe_100() {
    let kit = CanonicalKit::new();
    let output = parsed_module(&kit, 100);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);

    let start = Instant::now();
    let result = kit.eval(&output.stmts);
    let elapsed = start.elapsed();
    eprintln!("eval 100 pipes: {:?}", elapsed);
    let values = result.expect("eval should not error").values;
    assert!(!values.is_empty(), "eval produced no values");
    assert_eq!(values.last().unwrap().as_number(), Some(1.0));
    assert!(elapsed < Duration::from_millis(10), "eval 100 took {:?}", elapsed);
}

#[test]
fn eval_long_pipe_1000() {
    let kit = CanonicalKit::new();
    let output = parsed_module(&kit, 1000);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);

    let start = Instant::now();
    let result = kit.eval(&output.stmts);
    let elapsed = start.elapsed();
    eprintln!("eval 1000 pipes: {:?}", elapsed);
    let values = result.expect("eval should not error").values;
    assert!(!values.is_empty(), "eval produced no values");
    assert_eq!(values.last().unwrap().as_number(), Some(1.0));
    assert!(elapsed < Duration::from_millis(100), "eval 1000 took {:?}", elapsed);
}

#[test]
#[ignore = "10k nested fn.pipe overflows the default stack in debug builds"]
fn eval_long_pipe_10000() {
    let kit = CanonicalKit::new();
    let output = parsed_module(&kit, 10000);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);

    let start = Instant::now();
    let result = kit.eval(&output.stmts);
    let elapsed = start.elapsed();
    eprintln!("eval 10000 pipes: {:?}", elapsed);
    let values = result.expect("eval should not error").values;
    assert!(!values.is_empty(), "eval produced no values");
    assert_eq!(values.last().unwrap().as_number(), Some(1.0));
    assert!(elapsed < Duration::from_secs(1), "eval 10000 took {:?}", elapsed);
}
