use super::diagnostics::check_tu;
use crate::testutil::parse_snippet;

#[test]
fn clean_tu_passes() {
    let sn = parse_snippet("int add(int a, int b);\n");
    check_tu(sn.tu()).unwrap();
}

#[test]
fn parse_errors_fail_with_messages() {
    // `nosuchtype` is an unknown type name → error-severity diagnostic.
    let sn = parse_snippet("int f(nosuchtype x);\n");
    let err = check_tu(sn.tu()).unwrap_err();
    assert!(err.0.contains("parse errors"), "{err}");
    assert!(err.0.contains("error"), "{err}");
}
