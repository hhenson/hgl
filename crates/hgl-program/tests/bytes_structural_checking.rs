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
fn sparse(value:bytes)->Packet {when {return delta<Packet>(data:value)}}
test values {let value=bytes([0,255])
let retained_packet=packet(value)
let retained_pair=pair(value)
assert retained_packet.data==value
assert retained_pair[0]==value
assert retained_pair[1]==1}
test packet_copy {assert eval(packet_copy,[delta<Packet>(data:bytes([0,255]))])==[delta<Packet>(data:bytes([0,255]))]}
test pair_copy {assert eval(pair_copy,[(bytes([0,255]),1)])==[(bytes([0,255]),1)]}
test sparse {assert eval(sparse,[bytes(),bytes([0,255])])==[delta<Packet>(data:bytes()),delta<Packet>(data:bytes([0,255]))]}
",
    ));
    assert!(result.is_ok(), "{result:?}");
}
