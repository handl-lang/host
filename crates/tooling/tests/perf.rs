//! Wall-clock performance tests for the LSP / tooling host.
//!
//! Formatting and diagnostics live in `handl-tooling` because the formatter
//! plugins depend on `handl-kit-canonical`, making a reverse dependency cycle
//! impossible. The 10k-pipe format case is ignored because it overflows the
//! default debug stack.

use std::time::{Duration, Instant};

use handl_kit_canonical::CanonicalKit;
use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_fmt_expression::ExpressionFormatter;
use handl_tooling::ToolingHost;

fn host() -> ToolingHost {
    ToolingHost::new(
        CanonicalKit::new(),
        Box::new(ExpressionFormatter::new()),
        Box::new(ExpressionDiagnostics::new()),
    )
}

fn pipe_expr(n: usize) -> String {
    let mut s = "1".to_string();
    for _ in 0..n {
        s.push_str(" |> f");
    }
    s
}

fn large_let_file(n: usize) -> String {
    (0..n)
        .map(|i| format!("let a{} = {};", i, i))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn format_long_pipe_100() {
    let h = host();
    let input = pipe_expr(100);
    let start = Instant::now();
    let result = h.format(&input);
    let elapsed = start.elapsed();
    eprintln!("format 100 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "format failed: {:?}", result.err());
    assert!(elapsed < Duration::from_millis(5), "format 100 took {:?}", elapsed);
}

#[test]
fn format_long_pipe_1000() {
    let h = host();
    let input = pipe_expr(1000);
    let start = Instant::now();
    let result = h.format(&input);
    let elapsed = start.elapsed();
    eprintln!("format 1000 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "format failed: {:?}", result.err());
    assert!(elapsed < Duration::from_millis(150), "format 1000 took {:?}", elapsed);
}

#[test]
#[ignore = "10k nested fn.pipe overflows the default stack in debug builds"]
fn format_long_pipe_10000() {
    let h = host();
    let input = pipe_expr(10000);
    let start = Instant::now();
    let result = h.format(&input);
    let elapsed = start.elapsed();
    eprintln!("format 10000 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "format failed: {:?}", result.err());
    assert!(elapsed < Duration::from_secs(1), "format 10000 took {:?}", elapsed);
}

#[test]
fn lsp_diagnostics_long_500() {
    let h = host();
    let input = large_let_file(500);
    let start = Instant::now();
    let diags = h.diagnose(&input);
    let elapsed = start.elapsed();
    eprintln!("diagnose 500 let statements: {:?}", elapsed);
    assert!(elapsed < Duration::from_millis(500), "diagnose 500 took {:?}", elapsed);
    assert!(diags.is_empty(), "expected no diagnostics for benign lets, got {:?}", diags);
}
