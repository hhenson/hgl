//! BYTE-1–6 source admission and required constant evaluation boundaries.
use hgl_program::compile_tests;

fn sources(body: &str) -> Vec<(String, String)> {
    vec![
        ("bytes.hgl".into(), format!("module bytes_checking\n{body}")),
        (
            "replay.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ]
}

#[test]
fn byte_constructor_overloads_and_unadmitted_operations_are_rejected() {
    for expression in [
        "bytes(1)",
        "bytes(\"text\")",
        "bytes([true])",
        "bytes([1.0])",
        "bytes([],[])",
        "bytes(octets:[])",
        "bytes([0])[0]",
    ] {
        let error = compile_tests(&sources(&format!("test bad {{let value={expression}}}"))).err();
        assert!(error.is_some(), "{expression} unexpectedly admitted");
    }
}

#[test]
fn byte_constructor_does_not_extend_runtime_list_literal_admission() {
    let error = compile_tests(&sources(
        "fn bad(value:i64)->bytes {when {return bytes([value])}}\ntest bad {eval(bad,[1])}",
    ))
    .unwrap_err();
    assert!(error.contains("constant elements"), "{error}");
}

#[test]
fn required_constant_byte_failure_rejects_before_execution() {
    let error = compile_tests(&sources("const fn size()->i64 {return len(bytes([256]))}\nfn bad(value:list<i64,size()>)->list<i64,size()> {when {return delta_value(value)}}\ntest bad {eval(bad,[])}")).unwrap_err();
    assert!(error.contains("byte octet"), "{error}");
}
