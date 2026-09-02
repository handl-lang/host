use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_kit_canonical::Kit;

fn pipe_expr(n: usize) -> String {
    let mut s = "1".to_string();
    for _ in 0..n {
        s.push_str(" |> f");
    }
    s
}

fn pipe_module(n: usize) -> String {
    format!("f = x ~> x; {}", pipe_expr(n))
}

fn bench_parse(c: &mut Criterion) {
    let kit = Kit::new().expect("kit assembly should succeed");
    let mut group = c.benchmark_group("parse_long_pipe");
    for n in [100, 1000] {
        let input = pipe_expr(n);
        group.bench_with_input(format!("{}", n), &input, |b, input| {
            b.iter(|| kit.parse(black_box(input)).unwrap());
        });
    }
    group.finish();
}

fn bench_eval(c: &mut Criterion) {
    let kit = Kit::new().expect("kit assembly should succeed");
    let mut group = c.benchmark_group("eval_long_pipe");
    for n in [100, 1000] {
        let input = pipe_module(n);
        group.bench_with_input(format!("{}", n), &input, |b, input| {
            b.iter(|| kit.eval(black_box(input)).unwrap_or_default());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse, bench_eval);
criterion_main!(benches);
