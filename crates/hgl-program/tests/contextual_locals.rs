//! Initialized local categories follow the pinned specification independently of use.
use hgl_program::compile_tests;
use std::{fs, path::Path};

fn check(source: String) -> Result<(), String> {
    compile_tests(&[
        ("case.hgl".into(), source),
        (
            "replay.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ])
    .map(|_| ())
}

#[test]
fn pinned_local_category_errors_are_checking_errors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../external/hgraph_spec/compiler/contextual_bindings");
    for (name, args, diagnostic) in [
        ("scalar_to_port", "value:[1]", "category mismatch"),
        ("port_to_scalar", "value:[1]", "category mismatch"),
        ("unused_category_change", "value:[1]", "category mismatch"),
        ("let_scalar_reassign", "value:[1]", "writable var"),
        ("let_port_reassign", "first:[1],second:[2]", "writable var"),
        ("scalar_type_change", "value:[1]", "type mismatch"),
        (
            "port_type_change",
            "value:[1],other:[\"x\"]",
            "type mismatch",
        ),
        ("node_atomic_composite", "value:[1]", "tuple"),
    ] {
        let source = fs::read_to_string(root.join(format!("{name}.hgl"))).unwrap();
        let error = check(format!("{source}\ntest reject {{eval(sample,{args})}}")).unwrap_err();
        assert!(error.contains(diagnostic), "{name}: {error}");
    }
}

#[test]
fn declared_backend_boundaries_remain_explicit() {
    for (body, diagnostic) in [
        ("var local:i64\nlocal=1\nreturn value", "expected ="),
        (
            "var local=value\nif value > 0 {local=value}\nreturn local",
            "temporal graph conditionals are unsupported",
        ),
    ] {
        let source = format!(
            "module example\nfn sample(value:i64)->i64 {{{body}}}\ntest t {{eval(sample,value:[1])}}"
        );
        let error = check(source).unwrap_err();
        assert!(error.contains(diagnostic), "{error}");
    }
}

#[test]
fn compound_results_and_nested_writes_keep_the_initialized_category() {
    for (body, diagnostic) in [
        (
            "var local=value\nlocal=1\nreturn value",
            "category mismatch",
        ),
        (
            "var local=1\nlocal+=value\nreturn value",
            "category mismatch",
        ),
        ("let local=value\nlocal+=1\nreturn local", "writable var"),
        ("var local=value\nlocal+=1.5\nreturn value", "type mismatch"),
        (
            "var local=1\nif false {local=value}\nreturn value",
            "category mismatch",
        ),
        (
            "var local=1\nif false {let local=1\nlocal=2}\nreturn value",
            "writable var",
        ),
    ] {
        let source = format!(
            "module example\nfn sample(value:i64)->i64 {{{body}}}\ntest t {{eval(sample,value:[1])}}"
        );
        let error = check(source).unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
    }
}

#[test]
fn composite_ports_keep_exact_shapes_and_ordinary_observation_boundaries() {
    let source = "module example\nfn sample(a:atomic<tuple<i64,i64>>,b:atomic<tuple<i64,str>>)->atomic<tuple<i64,i64>> {var local=a\nlocal=b\nreturn a}\ntest t {eval(sample,a:[(1,2)],b:[(1,\"b\")])}";
    assert!(check(source.into()).unwrap_err().contains("type mismatch"));
    for (body, diagnostic) in [
        (
            "let local=value\nreturn local",
            "structural endpoint payload requires delta_value",
        ),
        (
            "let local:atomic<tuple<i64,i64>> = delta_value(value)\nreturn local",
            "local initializer type mismatch",
        ),
    ] {
        let source = format!(
            "module example\nfn sample(value:atomic<tuple<i64,i64>>)->atomic<tuple<i64,i64>> {{when {{{body}}}}}\ntest t {{eval(sample,value:[(1,2)])}}"
        );
        let error = check(source).unwrap_err();
        assert!(error.contains(diagnostic), "{error}");
    }
}

#[test]
fn graph_binary_operations_preserve_formal_signal_restrictions() {
    for body in [
        "return value+1",
        "let alias=value\nreturn alias+1",
        "var alias=value\nalias+=1\nreturn alias",
        "let first=value\nlet second=first\nreturn second+1",
    ] {
        let source = format!(
            "module example\nfn sample(value:signal)->i64 {{{body}}}\ntest t {{eval(sample,value:[1])}}"
        );
        assert!(check(source).unwrap_err().contains("signal"), "{body}");
    }
    let source = "module example\nfn signal_sink(value:signal) {when {}}\nfn signal_wrapper(value:signal) {signal_sink(value)}\nfn wrapper(value:i64)->i64 {signal_wrapper(value)\nreturn value+1}\ntest t {assert eval(wrapper,value:[1])==[2]}";
    assert!(check(source.into()).is_ok());
}

#[test]
fn ordinary_branches_check_every_connection_write_and_lexical_scope() {
    for (body, diagnostic) in [
        (
            "var r=x\nif false {r=y\nlet k=1\nlet k=2}\nreturn r",
            "duplicate local k",
        ),
        ("var r=x\nif false {r=1}\nreturn r", "category mismatch"),
        ("var r=x\nif false {r=y+1.5}\nreturn r", "type mismatch"),
        ("var r=x\nif false {let r=y\nr=x}\nreturn r", "writable var"),
        ("let r=x\nif true {r=y}\nreturn r", "writable var"),
        (
            "var r=x\nif true {r=y\nlet inner=x}\nreturn inner",
            "unknown value inner",
        ),
        (
            "var r=x\nif true {r=identity(y)}\nreturn r",
            "calls are unsupported",
        ),
    ] {
        let source = format!(
            "module example\nfn identity(value:i64)->i64 {{when {{return value}}}}\nfn sample(x:i64,y:i64)->i64 {{{body}}}\ntest t {{eval(sample,x:[1],y:[2])}}"
        );
        let error = check(source).unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
    }
    let result = check(include_str!("fixtures/contextual_locals.hgl").into());
    assert!(result.is_ok(), "{result:?}");
}
