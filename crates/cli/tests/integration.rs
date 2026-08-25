use std::process::Command;

#[test]
fn handl_fmt_runs() {
    let output = Command::new(env!("CARGO_BIN_EXE_handl"))
        .arg("fmt")
        .output()
        .expect("handl binary should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("formatted"), "{}", stdout);
    assert!(stdout.contains("true"), "{}", stdout);
}

#[test]
fn handl_eval_42_runs() {
    let output = Command::new(env!("CARGO_BIN_EXE_handl"))
        .args(["eval", "42"])
        .output()
        .expect("handl binary should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("42.0"), "{}", stdout);
}
