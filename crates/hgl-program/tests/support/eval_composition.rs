//! Closed source requirements determine internal recorder key selection.
use super::*;

fn plan(body: &str) -> Plan {
    let source = format!(
        "module example\nuse hgraph.std::{{TimedValue}}\nfn probe(value:i64)->i64 {{{body}}}"
    );
    let library = crate::index::load(&[
        ("example.hgl".into(), source),
        (
            "replay.hgl".into(),
            include_str!("../../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl")
                .into(),
        ),
    ])
    .unwrap();
    evaluate(
        library,
        "example",
        "probe",
        &[(
            None,
            Expr::Sequence(vec![Some(Expr::Literal(Literal::Int(1)))]),
        )],
    )
    .unwrap()
}
#[test]
fn fresh_keys_exclude_all_selected_requirements_regardless_of_type_or_execution() {
    let baseline = plan("when {return delta_value(value)}");
    let occupied = &baseline.recording.unwrap().0;
    for requirement in [
        format!(
            "when {{if false {{let absent:i64=get(global_state,{occupied:?})}}\nreturn delta_value(value)}}"
        ),
        format!("start {{set(global_state,{occupied:?},1)}}\nwhen {{return delta_value(value)}}"),
        format!("when {{set(global_state,{occupied:?},1)\nreturn delta_value(value)}}"),
        format!("when {{return delta_value(value)}}\nstop {{set(global_state,{occupied:?},1)}}"),
        format!(
            "start {{let empty:list<TimedValue<i64>> =[]\nset(global_state,{occupied:?},empty)}}\nwhen {{return delta_value(value)}}"
        ),
    ] {
        let actual = plan(&format!("inject global_state\n{requirement}"));
        assert_ne!(actual.recording.as_ref().unwrap().0, *occupied);
        assert!(
            actual
                .nodes
                .iter()
                .any(|node| node.globals.iter().any(|(key, _)| key == occupied))
        );
    }
}
#[test]
fn scalar_replay_and_record_use_ordinary_prepared_values() {
    let plan = plan("when {return delta_value(value)}");
    let replay = &plan.nodes[0];
    assert!(replay.generator.is_some());
    let Ty::List(entry, None) = &replay.configuration[0].ty else {
        panic!("ordinary unbounded list")
    };
    let Ty::Struct(identity, fields) = entry.as_ref() else {
        panic!("ordinary timed entry")
    };
    assert_eq!(identity.origin, "hgraph.std::TimedValue");
    assert_eq!(identity.arguments, vec![Ty::I64]);
    assert_eq!(
        fields,
        &vec![("time".into(), Ty::DateTime), ("value".into(), Ty::I64)]
    );
    let (key, ty) = plan.recording.as_ref().unwrap();
    let record = plan.nodes.last().unwrap();
    assert!(record.globals.contains(&(key.clone(), ty.clone())));
    assert_eq!(ty, &replay.configuration[0].ty);
}
