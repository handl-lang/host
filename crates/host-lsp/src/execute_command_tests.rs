use serde_json::json;

use crate::execute_command::execute_command;

#[test]
fn execute_command_unknown_returns_error() {
    let result = execute_command("handl.bogus", &[]);
    assert!(result.get("error").is_some(), "unknown command should error: {result:?}");
    let msg = result["error"].as_str().unwrap();
    assert!(msg.contains("unknown command"), "error message should mention unknown: {msg}");
}

#[test]
fn execute_command_run_arrow_returns_placeholder() {
    let args = vec![json!({ "uri": "file:///repo/arrows/sort.hnl" })];
    let result = execute_command("handl.runArrow", &args);
    let msg = result["result"].as_str().expect("runArrow should return a result string");
    assert!(
        msg.contains("Cannot run"),
        "runArrow should return a placeholder message: {msg}"
    );
}

#[test]
fn execute_command_format_file_returns_edits() {
    // A poorly-indented file should produce formatting edits.
    let source = "x = {\na: 1\n}";
    let args = vec![json!({ "source": source })];
    let result = execute_command("handl.formatFile", &args);
    assert!(result.get("result").is_some(), "formatFile should return a result: {result:?}");
    // The result is an array of edits (possibly empty if already formatted).
    let edits = result["result"].as_array().expect("edits array");
    assert!(
        !edits.is_empty(),
        "an unformatted file should produce at least one edit: {edits:?}"
    );
}

#[test]
fn execute_command_show_contract_on_non_arrow_returns_error() {
    // A plain `.hnl` file that is not an arrow file has no contract.
    let source = "x = 42";
    let args = vec![json!({ "source": source })];
    let result = execute_command("handl.showContract", &args);
    assert!(
        result.get("error").is_some(),
        "non-arrow file should yield an error for showContract: {result:?}"
    );
}

#[test]
fn execute_command_missing_source_returns_error() {
    // No arguments at all.
    let result = execute_command("handl.showContract", &[]);
    assert!(
        result.get("error").is_some(),
        "missing arguments should yield an error: {result:?}"
    );
}
