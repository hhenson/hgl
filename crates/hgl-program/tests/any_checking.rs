//! Readable ordinary payload authority precedes Any atomic wrapper lowering.
#[test]
fn signal_argument_cannot_become_a_readable_atomic_any_payload() {
    let source = "module signals\nconst fn items()->list<i64> {return []}\nfn source()->atomic<list<i64>> {inject alarm\nstart {schedule(alarm,0us)}\nwhen {return items()}}\nfn boxed(value:signal)->any {when {return any(value)}}\nexport fn main() {boxed(source())}";
    let error = hgl_program::compile(&[("signal.hgl".into(), source.into())], "main").unwrap_err();
    assert!(
        error.contains("signal has no ordinary scalar value"),
        "{error}"
    );
    let ordinary = source.replace("value:signal", "value:atomic<list<i64>>");
    let result = hgl_program::compile(&[("signal.hgl".into(), ordinary)], "main");
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn constructed_any_children_require_retained_structural_ownership() {
    let mut bodies=vec![
        "fn node(value:any)->tuple<any,i64> {when {return (value,1)}}".to_owned(),
        "fn node(value:any)->Cell {when {return Cell(value:value)}}".to_owned(),
        "fn node(value:any)->Cell {inject out\nwhen {out=Cell(value:value)}}".to_owned(),
        "struct Nested {cell:Cell}\nfn node(value:any)->Nested {when {let held=Nested(cell:Cell(value:value))\nreturn held}}".to_owned(),
    ];
    for shape in ["atomic<Cell>", "rolling<Cell,2>"] {
        for binding in ["let", "var"] {
            for publish in ["return held", "out=held"] {
                bodies.push(format!("fn node(value:any)->{shape} {{inject out\nwhen {{{binding} held=Cell(value:value)\n{publish}}}}}"));
            }
        }
        bodies.push(format!(
            "fn node(value:any)->{shape} {{when {{return Cell(value:value)}}}}"
        ));
    }
    for body in bodies {
        let source = format!(
            "module checking\nstruct Cell {{value:any}}\n{body}\ntest check {{eval(node,[any(1),any(2)])}}"
        );
        let result = hgl_program::compile_tests(&test_sources(source));
        let error = result.unwrap_err();
        assert!(
            error.contains("prepared retained observations"),
            "{body}: {error}"
        );
    }
    let source = "module checking\nstruct Cell {value:any}\nconst fn cell(value:any)->Cell {return Cell(value:value)}\nconst fn pair(value:any)->tuple<any,i64> {return (value,1)}\nfn copy(value:Cell)->Cell {when {return value}}\nfn sparse(value:any)->Cell {when {return delta<Cell>(value:value)}}\ntest check {assert cell(any(1)).value==any(1)\nassert pair(any(1))[0]==any(1)\nassert eval(copy,[delta<Cell>(value:any(1))])==[delta<Cell>(value:any(1))]\nassert eval(sparse,[any(1)])==[delta<Cell>(value:any(1))]}";
    let result = hgl_program::compile_tests(&test_sources(source.into()));
    assert!(result.is_ok(), "{result:?}");
}

fn test_sources(source: String) -> Vec<(String, String)> {
    vec![
        ("checking.hgl".into(), source),
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
