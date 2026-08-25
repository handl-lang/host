use handl_compiler::pipeline::PipelineBuilder;
use handl_compiler_tests::{full_pass_host, mock_host};
use serde_json::json;

#[test]
fn e2e_pipeline_runs_five_default_passes() {
    let host = full_pass_host();
    let pipeline = PipelineBuilder::new(&host).build();
    let result = pipeline.run(json!({"start": true})).unwrap();

    let generated = result.get("generated").expect("generate wraps output");
    assert!(generated.get("start").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(generated.get("parsed").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(generated.get("extracted").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(generated.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(generated.get("typechecked").and_then(|v| v.as_bool()).unwrap_or(false));
}

#[test]
fn pipeline_skips_unregistered_stages() {
    let host = mock_host("typecheck");
    let pipeline = PipelineBuilder::new(&host).build();
    let result = pipeline.run(json!({"start": true})).unwrap();

    assert!(result.get("start").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(result.get("typecheck").and_then(|v| v.as_bool()).unwrap_or(false));
    assert!(result.get("parsed").is_none());
    assert!(result.get("extracted").is_none());
    assert!(result.get("resolved").is_none());
}
