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

#[test]
fn byte_conversion_cannot_read_atomic_signal_payloads() {
    for shape in ["list<i64>", "list<i64,2>"] {
        for expression in ["bytes(value)", "convert(value)"] {
            for result in ["bytes", "rolling<bytes,2>"] {
                let readable = format!(
                    "const fn convert(value:{shape})->bytes {{return bytes(value)}}\nfn observe(value:atomic<{shape}>)->{result} {{when {{return {expression}}}}}\ntest accepted {{eval(observe,[[0,255]])}}"
                );
                compile_tests(&sources(&readable))
                    .unwrap_or_else(|error| panic!("{shape}, {expression}: {error}"));
                let body = format!(
                    "const fn convert(value:{shape})->bytes {{return bytes(value)}}\nfn observe(value:signal)->{result} {{when {{return {expression}}}}}\nfn target(value:atomic<{shape}>)->{result} => observe(value)\ntest rejected {{eval(target,[[0,255]])}}"
                );
                let error = compile_tests(&sources(&body)).unwrap_err();
                assert!(error.contains("signal"), "{shape}, {expression}: {error}");
            }
        }
    }
}

#[test]
fn retained_byte_locals_keep_mutability_and_signal_boundaries() {
    for initializer in ["value", "delta_value(value)"] {
        for result in ["bytes", "rolling<bytes,2>"] {
            let body = format!(
                "fn target(value:bytes)->{result} {{when {{var held={initializer}\nreturn held}}}}\ntest rejected {{eval(target,[bytes([0,255])])}}"
            );
            let error = compile_tests(&sources(&body)).unwrap_err();
            assert!(error.contains("mutable retained"), "{body}\n{error}");
            let body = format!(
                "fn observe(value:signal)->{result} {{when {{let held={initializer}\nreturn held}}}}\nfn target(value:bytes)->{result} => observe(value)\ntest rejected {{eval(target,[bytes([0,255])])}}"
            );
            let error = compile_tests(&sources(&body)).unwrap_err();
            assert!(error.contains("signal"), "{body}\n{error}");
        }
    }
}

#[test]
fn rolling_byte_arrival_retention_keeps_signal_and_mutability_boundaries() {
    for result in ["bytes", "rolling<bytes,2>"] {
        let body = format!(
            "fn observe(value:signal)->{result} {{when {{let held=delta_value(value)\nreturn held}}}}\nfn target(value:rolling<bytes,2>)->{result} => observe(value)\ntest rejected {{eval(target,[bytes([0,255])])}}"
        );
        let error = compile_tests(&sources(&body)).unwrap_err();
        assert!(error.contains("signal"), "{body}\n{error}");
        let body = format!(
            "fn target(value:rolling<bytes,2>)->{result} {{when {{var held=delta_value(value)\nreturn held}}}}\ntest rejected {{eval(target,[bytes([0,255])])}}"
        );
        let error = compile_tests(&sources(&body)).unwrap_err();
        assert!(error.contains("mutable retained"), "{body}\n{error}");
    }
}
