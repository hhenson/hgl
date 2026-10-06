//! Ordinary CLI test runs combine source probes and executable cases.
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn run(file: &Path, names: &[&str], parts: &[PathBuf]) -> std::io::Result<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hglc"));
    command.arg("test").arg(file).args(names);
    let stdlib =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../external/hgraph_std/hgl/hgraph");
    for name in [
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
        command.arg("--part").arg(stdlib.join(name));
    }
    for part in parts {
        command.arg("--part").arg(part);
    }
    command
        .arg("--part")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../native/stdlib/rust.hgl"));
    command
        .arg("--part")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../native/stdlib/interfaces.hgl"));
    command.output()
}
fn spec(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../external/hgraph_spec")
        .join(relative)
}
#[test]
fn mixed_script_reports_both_kinds_and_never_executes_rejected_test() {
    let output = run(&spec("language/examples/reject/mixed.hgl"), &[], &[]).unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("before_rejections ... ok [executed]"));
    assert!(text.contains("after_rejections ... ok [executed]"));
    assert!(text.contains("rejected_test ... ok [rejection]"));
    assert!(!text.contains("rejected_test ... ok [executed]"));
    assert!(text.contains("2 executed tests"));
    assert!(text.contains("2 rejection cases, 0 failures"));
}
#[test]
fn mismatch_continues_execution_but_unsafe_admission_does_not() {
    let output = run(
        &spec("compiler/negative_testing/controls/mixed-rejection-mismatch.hgl"),
        &[],
        &[],
    )
    .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(text.contains("... FAILED [rejection]"));
    assert!(text.contains("runtime_sentinel ... ok [executed]"));
    for file in [
        "unsafe-recovery",
        "unnamed-context-header",
        "dependency-on-excluded",
        "invalid-metadata-unselected",
    ] {
        let output = run(
            &spec(&format!("compiler/negative_testing/controls/{file}.hgl")),
            &[],
            &[],
        )
        .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("[executed]"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("admission:"));
    }
}
#[test]
fn selectors_and_parts_apply_to_both_case_kinds() {
    let main = spec("language/examples/reject/mixed-parts/main.hgl");
    let parts = [
        spec("language/examples/reject/mixed-parts/helpers.hgl"),
        spec("language/examples/reject/mixed-parts/cases.hgl"),
    ];
    let output = run(&main, &["rejected_part_test"], &parts).unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("rejected_part_test ... ok [rejection]"));
    assert!(!text.contains("[executed]"));
    assert!(text.contains("2 rejection cases"));
}
#[test]
fn rejection_only_does_not_need_a_toolchain_or_special_flag() {
    let path = spec("language/examples/reject/rolling-size-kind.hgl");
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("test")
        .arg(&path)
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let removed = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .args(["test", "--reject"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!removed.status.success());
    assert!(removed.stdout.is_empty());
}

#[test]
fn temporary_directory_failure_is_infrastructure_not_a_test_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("test")
        .arg(spec("language/examples/reject/mixed.hgl"))
        .env("TMPDIR", spec("language/examples/reject/mixed.hgl"))
        .env("TEMP", spec("language/examples/reject/mixed.hgl"))
        .env("TMP", spec("language/examples/reject/mixed.hgl"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("infrastructure:"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("[executed]"));
}

#[test]
fn library_tests_keep_their_own_module_and_production_dependencies_are_checked() {
    let dir = std::env::temp_dir().join(format!("hgl-module-selection-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library")).unwrap();
    let main = dir.join("main.hgl");
    let dependency = dir.join("library/dependency.hgl");
    std::fs::write(&main, "module target\ntest selected {assert true}\n").unwrap();
    std::fs::write(&dependency,"module dependency\n# expect-error(type, \"unknown\")\nfn identity<T>(value:T)->T=>value\ntest foreign {assert false}\ntest {fn unselected<T>(value:T)->T=>unknown(value)}\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("test")
        .arg(&main)
        .arg("--library")
        .arg(dir.join("library"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("target::selected ... ok [executed]"));
    assert!(!text.contains("foreign"));
    std::fs::write(
        &dependency,
        "module dependency\nfn invalid<T>(value:T)->T=>unknown(value)\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("test")
        .arg(&main)
        .arg("--library")
        .arg(dir.join("library"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("admission:"));
    assert!(output.stdout.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
