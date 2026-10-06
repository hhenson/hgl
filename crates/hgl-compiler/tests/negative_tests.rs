//! Public CLI execution distinguishes a failed test from failed compilation.
use std::{path::PathBuf, process::Command};
fn run(path: &std::path::Path, selectors: &[&str]) -> std::io::Result<std::process::Output> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_hglc"));
    command.arg("test").arg(path).args(selectors);
    for part in [
        "replay_record.hgl",
        "impl/replay_record.hgl",
        "control.hgl",
        "impl/control.hgl",
        "standard.hgl",
        "impl/standard.hgl",
        "native/scalar_values.hgl",
        "native/scalar_values_i64.hgl",
        "native/scalar_operators.hgl",
        "native/temporal_values.hgl",
    ] {
        command
            .arg("--part")
            .arg(root.join("external/hgraph_std/hgl/hgraph").join(part));
    }
    command
        .arg("--part")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../native/stdlib/rust.hgl"));
    command
        .arg("--part")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../native/stdlib/interfaces.hgl"));
    command.output()
}
#[test]
fn shared_runtime_assertions_execute_and_select_names() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/negative-tests/runtime.hgl");
    let output = run(
        &path,
        &["expected_execution_errors", "nested_execution_errors"],
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("2 executed tests, 6 evaluations, 0 failures")
    );
    let output = run(&path, &["expected_execution_errors", "missing"]).unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown test selector missing"));
}
#[test]
fn failed_assertions_execute_but_compile_failures_never_do() {
    let path = std::env::temp_dir().join(format!("hgl-negative-cli-{}.hgl", std::process::id()));
    std::fs::write(
        &path,
        r#"module controls
fn negative()->i64 {yield -1us:1}
fn consume(tick:i64) {when {}}
fn drive(tick:i64)->i64 {consume(tick)
negative()}
test empty {assert raises("yield.negative_duration") {}}
test assertion {assert raises("yield.negative_duration") {assert false}}
test wrong {assert raises("yield.non_increasing_time") {eval(drive,tick:[1,1])}}
test nested {assert raises("yield.negative_duration") {assert raises("yield.negative_duration") {}}}
"#,
    )
    .unwrap();
    let output = run(&path, &[]).unwrap();
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for name in ["empty", "assertion", "wrong", "nested"] {
        assert!(
            text.contains(&format!("controls::{name} ... FAILED")),
            "{text}"
        );
    }
    std::fs::write(
        &path,
        r#"module invalid
test invalid {assert raises("not.a.code") {}}
"#,
    )
    .unwrap();
    let output = run(&path, &[]).unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("... FAILED"));
    std::fs::remove_file(path).unwrap();
}
