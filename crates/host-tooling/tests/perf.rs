//! Wall-clock performance tests for the LSP / tooling host.
//!
//! These are ignored while `ToolingHost.format` and `diagnostics` are stubbed.

use std::time::{Duration, Instant};

use handl_host_tooling::ToolingHost;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};

/// Minimal stub kit plugin (replaces the removed `CanonicalKit`).
struct StubKit;

impl Plugin for StubKit {
    fn id(&self) -> &str {
        "stub.kit"
    }
    fn name(&self) -> &str {
        "stub-kit"
    }
    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }
    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }
    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
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

fn host() -> ToolingHost<StubKit> {
    ToolingHost::new(StubKit)
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
        .map(|i| format!("a{} = {};", i, i))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
#[ignore = "format not yet implemented"]
fn format_long_pipe_100() {
    let h = host();
    let input = pipe_expr(100);
    let start = Instant::now();
    let result = h.format(&input);
    let elapsed = start.elapsed();
    eprintln!("format 100 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "format failed: {:?}", result.err());
    assert!(
        elapsed < Duration::from_millis(5),
        "format 100 took {:?}",
        elapsed
    );
}

#[test]
#[ignore = "format not yet implemented"]
fn format_long_pipe_1000() {
    let h = host();
    let input = pipe_expr(1000);
    let start = Instant::now();
    let result = h.format(&input);
    let elapsed = start.elapsed();
    eprintln!("format 1000 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "format failed: {:?}", result.err());
    assert!(
        elapsed < Duration::from_millis(150),
        "format 1000 took {:?}",
        elapsed
    );
}

#[test]
#[ignore = "10k nested fn.pipe overflows the default stack in debug builds; format not yet implemented"]
fn format_long_pipe_10000() {
    let h = host();
    let input = pipe_expr(10000);
    let start = Instant::now();
    let result = h.format(&input);
    let elapsed = start.elapsed();
    eprintln!("format 10000 pipes: {:?}", elapsed);
    assert!(result.is_ok(), "format failed: {:?}", result.err());
    assert!(
        elapsed < Duration::from_secs(1),
        "format 10000 took {:?}",
        elapsed
    );
}

#[test]
#[ignore = "diagnostics not yet implemented"]
fn lsp_diagnostics_long_500() {
    let h = host();
    let input = large_let_file(500);
    let start = Instant::now();
    let diags = h.diagnose(&input);
    let elapsed = start.elapsed();
    eprintln!("diagnose 500 let statements: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(500),
        "diagnose 500 took {:?}",
        elapsed
    );
    assert!(
        diags.is_empty(),
        "expected no diagnostics for benign lets, got {:?}",
        diags
    );
}
