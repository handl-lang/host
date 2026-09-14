//! Criterion benchmark for the LLVM compiler.
//!
//! Since `inkwell` is off by default, this measures the placeholder compile
//! path. The harness is real; full measurement requires the `inkwell` feature.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_language::{Ast, AstNode, Language};
use handl_plugin_compile::{Compiler, CompilerInput};
use handl_plugin_compile_llvm::{LlvmCompiler, LlvmConfig, X86_64};
use plugin_engine::Term;
use plugin_engine_typecheck::TypecheckReport;
use serde_json::Value;

fn make_input(term: Term) -> CompilerInput {
    let source = "bench".to_string();
    let mut ast = Ast::new();
    let root = ast.push(AstNode::new("module"));
    let language = Language::new();
    let report = TypecheckReport::default();
    CompilerInput::new(source, ast, root, language, term, report)
}

fn bench_compile_null(c: &mut Criterion) {
    let compiler = LlvmCompiler::new();
    let cfg = LlvmConfig::new(Box::new(X86_64));
    let input = make_input(Term::constant(Value::Null));
    c.bench_function("compile_null_const", |b| {
        b.iter(|| {
            black_box(
                compiler
                    .compile(black_box(&input), black_box(&cfg))
                    .unwrap(),
            )
        });
    });
}

criterion_group!(benches, bench_compile_null);
criterion_main!(benches);
