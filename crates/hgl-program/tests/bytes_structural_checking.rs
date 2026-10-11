//! Owning byte children obey the existing prepared structural publication boundary.
use hgl_program::compile_tests;

fn sources(body: &str) -> Vec<(String, String)> {
    vec![
        (
            "bytes_structural.hgl".into(),
            format!("module bytes_structural\nstruct Packet {{data:bytes}}\n{body}"),
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
fn native_byte_children_require_prepared_structural_ownership() {
    for body in [
        "fn bad(value:bytes)->tuple<bytes,i64> {when {return (value,1)}}",
        "fn bad(value:bytes)->tuple<bytes,i64> {inject out\nwhen {out=(value,1)}}",
        "fn bad(value:bytes)->Packet {when {return Packet(data:value)}}",
        "fn bad(value:bytes)->Packet {inject out\nwhen {out=Packet(data:value)}}",
        "fn bad(value:bytes)->atomic<Packet> {when {return Packet(data:value)}}",
        "fn bad(value:bytes)->atomic<Packet> {inject out\nwhen {out=Packet(data:value)}}",
        "fn bad(value:bytes)->rolling<Packet,2> {when {return Packet(data:value)}}",
        "fn bad(value:bytes)->rolling<Packet,2> {inject out\nwhen {out=Packet(data:value)}}",
        "struct Envelope {packet:Packet}\nfn bad(value:bytes)->Envelope {when {return Envelope(packet:Packet(data:value))}}",
        "fn bad(value:bytes)->Packet {when {let packet=Packet(data:value)\nreturn packet}}",
    ] {
        let source = format!("{body}\ntest bad {{eval(bad,[bytes([0,255]),bytes()])}}");
        let error = compile_tests(&sources(&source))
            .expect_err("native byte child publication lacks prepared ownership");
        assert!(
            error.contains("prepared retained observations"),
            "{source}\n{error}"
        );
    }
}

#[test]
fn native_byte_struct_aliases_require_prepared_result_ownership() {
    for shape in ["atomic<Packet>", "rolling<Packet,2>"] {
        for binding in ["let", "var"] {
            for publication in ["return packet", "out=packet"] {
                let source = format!(
                    "fn bad(value:bytes)->{shape} {{inject out\nwhen {{{binding} packet=Packet(data:value)\n{publication}}}}}\ntest bad {{eval(bad,[bytes([0,255])])}}"
                );
                let error = compile_tests(&sources(&source))
                    .expect_err("native owning alias has no prepared result transport");
                assert!(
                    error.contains("prepared retained observations"),
                    "{source}\n{error}"
                );
            }
        }
    }
    let error = compile_tests(&sources(
        "fn bad(value:rolling<Packet,2>)->rolling<Packet,2> {when {let packet=delta_value(value)\nreturn packet}}\ntest bad {eval(bad,[Packet(data:bytes([0,255]))])}",
    )).expect_err("native rolling arrival alias has no prepared result transport");
    assert!(error.contains("prepared retained observations"), "{error}");
}

#[test]
fn retained_byte_projections_require_prepared_helper_ownership() {
    for (shape, argument, expression) in [
        ("tuple<bytes,i64>", "(bytes([0,255]),1)", "value[0]"),
        ("tuple<bytes,i64>", "(bytes([0,255]),1)", "held[0]"),
        ("Packet", "delta<Packet>(data:bytes([0,255]))", "value.data"),
        ("Packet", "delta<Packet>(data:bytes([0,255]))", "held.data"),
    ] {
        let source = format!(
            "const fn size(value:bytes)->i64 {{return len(value)}}\nfn bad(value:{shape})->i64 {{when {{let held=value\nreturn size({expression})}}}}\ntest bad {{eval(bad,[{argument}])}}"
        );
        let error = compile_tests(&sources(&source))
            .expect_err("retained byte projections lack a prepared ordinary helper ABI");
        assert!(
            error.contains("owning helper arguments require prepared storage"),
            "{source}\n{error}"
        );
    }
}

#[test]
fn ordinary_byte_construction_and_retained_structural_values_remain_admitted() {
    let result = compile_tests(&sources(
        r"const fn packet(value:bytes)->Packet {return Packet(data:value)}
const fn pair(value:bytes)->tuple<bytes,i64> {return (value,1)}
fn packet_copy(value:Packet)->Packet {when {return value}}
fn pair_copy(value:tuple<bytes,i64>)->tuple<bytes,i64> {when {return value}}
fn atomic_observation(value:atomic<Packet>)->atomic<Packet> {when {let packet=delta_value(value)
return packet}}
fn sparse(value:bytes)->Packet {when {return delta<Packet>(data:value)}}
test values {let value=bytes([0,255])
let retained_packet=packet(value)
let retained_pair=pair(value)
assert retained_packet.data==value
assert retained_pair[0]==value
assert retained_pair[1]==1}
test packet_copy {assert eval(packet_copy,[delta<Packet>(data:bytes([0,255]))])==[delta<Packet>(data:bytes([0,255]))]}
test pair_copy {assert eval(pair_copy,[(bytes([0,255]),1)])==[(bytes([0,255]),1)]}
test atomic_observation {assert eval(atomic_observation,[Packet(data:bytes([0,255]))])==[Packet(data:bytes([0,255]))]}
test sparse {assert eval(sparse,[bytes(),bytes([0,255])])==[delta<Packet>(data:bytes()),delta<Packet>(data:bytes([0,255]))]}
",
    ));
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn retained_byte_constructors_keep_whole_aggregate_result_boundaries() {
    for (result, constructor) in [
        ("atomic<Packet>", "Packet(data:held)"),
        ("rolling<Packet,2>", "Packet(data:held)"),
        ("atomic<tuple<bytes,i64>>", "(held,1)"),
        ("rolling<tuple<bytes,i64>,2>", "(held,1)"),
    ] {
        for publication in [
            format!("return {constructor}"),
            format!("out={constructor}"),
        ] {
            let body = format!(
                "fn bad(value:bytes)->{result} {{inject out\nwhen {{let held=value\n{publication}}}}}\ntest rejected {{eval(bad,[bytes([0,255])])}}"
            );
            let error = compile_tests(&sources(&body))
                .err()
                .unwrap_or_else(|| panic!("unexpected whole-arrival admission: {body}"));
            assert!(error.contains("retained"), "{body}\n{error}");
        }
    }
}

#[test]
fn byte_field_locals_keep_signal_mutability_and_selector_boundaries() {
    for (shape, projection, argument) in [
        ("Packet", "value.data", "delta<Packet>(data:bytes([0,255]))"),
        ("tuple<bytes,i64>", "value[0]", "(bytes([0,255]),1)"),
        (
            "list<bytes,2>",
            "value[0]",
            "delta<list<bytes,2>>(items:[0:bytes([0,255])])",
        ),
    ] {
        for result in ["bytes", "rolling<bytes,2>"] {
            let body = format!(
                "fn bad(value:{shape})->{result} {{when {{var held={projection}\nreturn held}}}}\ntest rejected {{eval(bad,[{argument}])}}"
            );
            let error = compile_tests(&sources(&body)).unwrap_err();
            assert!(error.contains("mutable retained"), "{body}\n{error}");
            let body = format!(
                "fn observe(value:signal)->{result} {{when {{let held={projection}\nreturn held}}}}\nfn bad(value:{shape})->{result} => observe(value)\ntest rejected {{eval(bad,[{argument}])}}"
            );
            let error = compile_tests(&sources(&body)).unwrap_err();
            assert!(error.contains("signal"), "{body}\n{error}");
        }
        let body = format!(
            "fn bad(value:{shape})->bytes {{when {{let held=delta_value({projection})\nreturn held}}}}\ntest rejected {{eval(bad,[{argument}])}}"
        );
        let error = compile_tests(&sources(&body)).unwrap_err();
        assert!(error.contains("temporal input endpoint"), "{body}\n{error}");
    }
    let body = "fn bad(value:list<bytes,2>,index:i64)->bytes {when {let held=value[index]\nreturn held}}\ntest rejected {eval(bad,[delta<list<bytes,2>>(items:[0:bytes()])],[0])}";
    let error = compile_tests(&sources(body)).unwrap_err();
    assert!(error.contains("constant integer position"), "{error}");
}

#[test]
fn byte_field_snapshots_require_finite_preparation() {
    for result in ["bytes", "rolling<bytes,2>"] {
        for publication in ["return held", "out=held"] {
            let body = format!(
                "fn source()->Packet {{yield 0s:delta<Packet>(data:bytes([0,255]))}}\nfn copy(value:Packet)->{result} {{inject out\nwhen {{let held=value.data\n{publication}}}}}\nfn main()->{result} => copy(source())"
            );
            let error = hgl_program::compile(&sources(&body), "main").unwrap_err();
            assert!(
                error.contains("owning Tuple observations require finite prepared transport"),
                "{body}\n{error}"
            );
        }
    }
}
