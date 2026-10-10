//! Published ordinary Tuple order is observable through the existing logger host.
use std::{path::Path, process::Command};

#[test]
fn published_tuple_examples_log_elements_in_written_order() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("test")
        .arg(root.join("external/hgraph_spec/language/examples/ordinary-tuple-values.hgl"))
        .args([
            "ordinary_tuple_written_order",
            "ordinary_tuple_nested_written_order",
        ])
        .arg("--library")
        .arg(root.join("external/hgraph_std/hgl/hgraph"))
        .arg("--part")
        .arg(root.join("native/stdlib/interfaces.hgl"))
        .arg("--part")
        .arg(root.join("native/stdlib/rust.hgl"))
        .output()
        .expect("execute the standard Rust source-test host");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    let messages = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("INFO "))
        .collect::<Vec<_>>();
    assert_eq!(messages, ["2", "1", "3", "2", "1"]);
    assert!(
        stdout.contains("2 executed tests, 2 evaluations, 0 failures"),
        "{stdout}"
    );
}

#[test]
fn effectful_helpers_cannot_supply_a_fixed_logger_label() {
    let source = "module effectful_label\nconst fn label()->str {inject logger\ninfo(logger,\"label\")\nreturn \"2\"}\nconst fn mark(value:i64,const message:str)->i64 {inject logger\ninfo(logger,message)\nreturn value}\nfn bad(value:i64)->tuple<i64,i64> {when {return(mark(value,label()),0)}}\nfn source()->i64 {yield 0us:1}\nfn main()->tuple<i64,i64> => bad(source())";
    let error = hgl_program::compile(&[("effectful_label.hgl".into(), source.into())], "main")
        .expect_err("an effectful call cannot become a fixed formal");
    assert!(
        error.contains("fixed argument requires a wiring-time value"),
        "{error}"
    );
    assert!(!error.contains("delta.type_mismatch"), "{error}");
}

#[test]
fn used_nonconstant_text_formals_keep_their_owned_abi() {
    let output = Command::new(env!("CARGO_BIN_EXE_hglc"))
        .arg("test")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/tuple_logger_host_controls.hgl"),
        )
        .arg("--library")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../external/hgraph_std/hgl/hgraph"))
        .arg("--part")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/stdlib/interfaces.hgl"))
        .arg("--part")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/stdlib/rust.hgl"))
        .output()
        .expect("execute ordinary text argument ownership control");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("1 executed tests, 1 evaluations, 0 failures")
    );
}
