//! Literal preparation is deterministic and cannot execute calls or provider recipes.
use hgl_rust_ir::{Kind, Node, Plan, Statement, Value};
use hgl_source::{FamilyType, Literal, Ty};
#[test]
fn preparation_is_idempotent_and_preserves_nonliteral_hook_phase() -> Result<(), String> {
    let member = Ty::Struct(
        "test::Member".into(),
        vec![("value".into(), Ty::I64)],
        Vec::new(),
    );
    let ty = Ty::Family(FamilyType::new(
        "test::Event".into(),
        Vec::new(),
        vec![(member.structure()?.0.clone(), member.clone())],
    )?);
    let payload = Value::new(
        ty.clone(),
        Kind::Unary(
            "family".into(),
            Box::new(Value::new(
                member,
                Kind::Construct(vec![(
                    0,
                    Value::new(Ty::I64, Kind::Literal(Literal::Int(2))),
                )]),
            )),
        ),
    );
    let plan = Plan {
        nodes: vec![Node {
            name: "test::source".into(),
            inputs: Vec::new(),
            result: ty.clone().atomic(),
            alarm: false,
            generator: None,
            start: Vec::new(),
            global_state: false,
            globals: Vec::new(),
            configuration: Vec::new(),
            stop: Vec::new(),
            caches: Vec::new(),
            handlers: vec![(
                None,
                vec![
                    Statement::Return(payload),
                    Statement::Return(Value::new(ty, Kind::Native(0, Vec::new()))),
                ],
            )],
        }],
        ..Plan::default()
    };
    let first = hgl_rust_families::prepare(&plan);
    let second = hgl_rust_families::prepare(&first);
    assert_eq!(first.nodes[0].configuration.len(), 1);
    assert_eq!(second.nodes[0].configuration.len(), 1);
    assert!(matches!(
        second.nodes[0].handlers[0].1[0],
        Statement::Return(Value {
            kind: Kind::Configuration(0),
            ..
        })
    ));
    assert!(matches!(
        second.nodes[0].handlers[0].1[1],
        Statement::Return(Value {
            kind: Kind::Native(0, _),
            ..
        })
    ));
    Ok(())
}
