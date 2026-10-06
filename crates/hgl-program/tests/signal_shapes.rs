//! Signal parameters observe metadata while preserving hidden concrete shapes.
use hgl_program::compile_tests;

fn sources(body: &str) -> Vec<(String, String)> {
    vec![
        (
            "signals.hgl".into(),
            format!("module signals\nstruct Fields {{amount:i64}}\n{body}"),
        ),
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
fn structural_signal_inference_never_treats_the_formal_as_ordinary_data() {
    for (shape, values) in [
        ("list<i64,2>", "[delta<list<i64,2>>(items:[1:0])]"),
        ("tuple<i64,str>", "[(0,_)]"),
        ("Fields", "[delta<Fields>(amount:0)]"),
        ("map<i64,i64>", "[delta<map<i64,i64>>(upsert:[1:0])]"),
        ("atomic<list<i64>>", "[[]]"),
    ] {
        let body = format!(
            "fn observe(value:signal)->bool {{when {{return modified(value)}}}}\nfn forward<T>(value:T)->bool => observe(value)\nfn target(value:{shape})->bool => forward(value)\ntest accepted {{eval(target,{values})}}"
        );
        compile_tests(&sources(&body)).unwrap_or_else(|error| panic!("{shape}: {error}"));
    }
}

#[test]
fn concrete_signal_shapes_never_expose_payloads() {
    for (shape, values, expression) in [
        ("i64", "[1]", "value"),
        ("i64", "[1]", "delta_value(value)"),
        ("Fields", "[delta<Fields>(amount:0)]", "value.amount"),
        (
            "list<i64,2>",
            "[delta<list<i64,2>>(items:[1:0])]",
            "value[0]",
        ),
        ("atomic<Fields>", "[Fields(amount:1)]", "value.amount"),
    ] {
        let body = format!(
            "fn observe(value:signal)->i64 {{when {{return {expression}}}}}\nfn target(value:{shape})->i64 => observe(value)\ntest rejected {{eval(target,{values})}}"
        );
        let error = compile_tests(&sources(&body)).unwrap_err();
        assert!(error.contains("signal"), "{shape}: {error}");
    }
}
