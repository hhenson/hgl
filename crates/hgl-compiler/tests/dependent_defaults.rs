//! Generic default inference inspects checked argument types without repeating effects.
use std::{path::Path, process::Command};
#[test]
fn inference_keeps_effectful_operands_once_in_written_order() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("test")
        .arg(root.join("crates/hgl-compiler/tests/fixtures/dependent_defaults.hgl"))
        .arg("--library")
        .arg(root.join("external/hgraph_std/hgl/hgraph"))
        .arg("--library")
        .arg(root.join("native/stdlib"))
        .output()
        .expect("execute authored default-inference source");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    let effects = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("INFO "))
        .collect::<Vec<_>>();
    assert_eq!(effects, ["2", "1", "2", "1"]);
    assert!(
        stdout.contains("1 executed tests, 1 evaluations, 0 failures"),
        "{stdout}"
    );
}
