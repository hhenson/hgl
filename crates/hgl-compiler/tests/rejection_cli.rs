//! Compile rejection uses the ordinary diagnostics and never invokes a build.
use std::{path::PathBuf, process::Command};
#[test]
fn shared_rejection_fixtures_and_false_positive_controls() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec = root.join("external/hgraph_spec");
    for (directory, names, expected) in [
        (
            "language/examples/reject",
            &[
                "missing-parameter-close",
                "raises-computed-code",
                "raises-unknown-code",
                "rolling-size-bounds",
                "rolling-size-kind",
                "test-inject",
                "yield-time-type",
                "comment-text",
            ][..],
            true,
        ),
        (
            "compiler/negative_testing/controls",
            &[
                "reject-valid-source",
                "reject-wrong-code",
                "reject-wrong-location",
                "reject-unexpected-error",
                "reject-malformed-annotation",
                "reject-unknown-category",
                "reject-unknown-code",
                "reject-category-mismatch",
                "reject-build-category",
                "reject-no-expectation",
            ][..],
            false,
        ),
    ] {
        for name in names {
            let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
                .arg("test")
                .arg(spec.join(directory).join(format!("{name}.hgl")))
                .output()
                .unwrap();
            assert_eq!(
                output.status.success(),
                expected,
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if expected {
                assert!(String::from_utf8_lossy(&output.stdout).contains("[rejection]"));
            }
        }
    }
}
#[test]
fn ordinary_check_never_treats_an_expectation_as_permission_to_reject() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = root.join("external/hgraph_spec/language/examples/reject/rolling-size-kind.hgl");
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("check")
        .arg(&file)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("rolling.size_kind"));
    for suffix in [&["missing_test"][..], &["--part", "unused.hgl"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
            .arg("test")
            .arg(&file)
            .args(suffix)
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}
