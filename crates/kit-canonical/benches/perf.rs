use criterion::{black_box, criterion_group, criterion_main, Criterion};
use handl_kit_canonical::CanonicalKit;

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

fn bench_parse(c: &mut Criterion) {
    let kit = CanonicalKit::new();
    let mut group = c.benchmark_group("parse_long_pipe");
    for n in [100, 1000] {
        let input = pipe_expr(n);
        group.bench_with_input(format!("{}", n), &input, |b, input| {
            b.iter(|| kit.parse(black_box(input)));
        });
    }
    group.finish();
}

fn bench_typecheck(c: &mut Criterion) {
    let kit = CanonicalKit::new();
    let mut group = c.benchmark_group("typecheck_long_pipe");
    for n in [100, 1000] {
        let input = pipe_module(n);
        let output = kit.parse_module_recovered(&input);
        assert!(output.diagnostics.is_empty(), "parse errors at {}: {:?}", n, output.diagnostics);
        group.bench_with_input(format!("{}", n), &output.stmts, |b, stmts| {
            b.iter(|| kit.typecheck(black_box(stmts)));
        });
    }
    group.finish();
}

fn bench_eval(c: &mut Criterion) {
    let kit = CanonicalKit::new();
    let mut group = c.benchmark_group("eval_long_pipe");
    for n in [100, 1000] {
        let input = pipe_module(n);
        let output = kit.parse_module_recovered(&input);
        assert!(output.diagnostics.is_empty(), "parse errors at {}: {:?}", n, output.diagnostics);
        group.bench_with_input(format!("{}", n), &output.stmts, |b, stmts| {
            b.iter(|| kit.eval(black_box(stmts)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse, bench_typecheck, bench_eval);
criterion_main!(benches);
