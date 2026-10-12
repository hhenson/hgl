//! Synthetic owners cannot reuse generator or helper declaration slots.
use hgl_semantics::{
    call_locals,
    ir::{Kind, Node, Plan, Statement, Value},
    snapshot_locals,
};
use hgl_source::{Literal, Ty};
fn boxed() -> Value {
    let mut value = Value::new(
        Ty::Any,
        Kind::Construct(vec![(
            0,
            Value::new(Ty::I64, Kind::Literal(Literal::Int(7))),
        )]),
    );
    value.snapshot = true;
    value
}
#[test]
fn generator_and_helper_owners_keep_independent_reserved_destinations() {
    for helper_owner in [false, true] {
        let mut body = Vec::new();
        if helper_owner {
            body.push(Statement::Let(0, boxed()));
        }
        body.push(Statement::Yield(Value::new(
            Ty::Bool,
            Kind::Literal(Literal::Bool(true)),
        )));
        let mut plan = Plan {
            nodes: vec![Node {
                name: "generator".into(),
                inputs: Vec::new(),
                result: Ty::Any,
                alarm: true,
                start: Vec::new(),
                stop: Vec::new(),
                handlers: Vec::new(),
                globals: Vec::new(),
                global_state: false,
                configuration: Vec::new(),
                caches: Vec::new(),
                generator: Some(vec![
                    Statement::Var(0, boxed()),
                    Statement::Let(9, Value::new(Ty::Bool, Kind::ValueCall(Vec::new(), body))),
                    Statement::Assign(Value::new(Ty::Any, Kind::MutableLocal(0)), boxed()),
                    Statement::TimedYield(
                        Value::new(Ty::Duration, Kind::Literal(Literal::Duration(0))),
                        Value::new(Ty::Any, Kind::MutableLocal(0)),
                    ),
                ]),
            }],
            ..Plan::default()
        };
        call_locals::prepare(&mut plan);
        snapshot_locals::prepare(&mut plan, Clone::clone);
        let node = &plan.nodes[0];
        let keys: Vec<_> = node.globals.iter().map(|(key, _)| key.as_str()).collect();
        let expected = if helper_owner {
            vec!["\0snapshot:0:0", "\0snapshot:0:10", "\0snapshot:0:11"]
        } else {
            vec!["\0snapshot:0:0", "\0snapshot:0:10"]
        };
        assert_eq!(keys, expected);
        let generator = node.generator.as_ref().unwrap_or_else(|| unreachable!());
        let Statement::Assign(target, value) = &generator[3] else {
            unreachable!()
        };
        assert!(matches!(target.kind, Kind::MutableLocal(0)));
        assert!(matches!(value.kind, Kind::Local(id) if id == if helper_owner {11} else {10}));
    }
}
