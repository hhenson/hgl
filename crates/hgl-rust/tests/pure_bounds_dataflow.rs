//! Independent branches must not multiply one another's required text width.
use hgl_semantics::ir::{Kind, Node, Plan, Statement, Value};
use hgl_source::Ty;
fn node(source: Option<usize>) -> Node {
    let value = Value::new(Ty::Str, Kind::Input(0, false));
    Node {
        name: "text".into(),
        inputs: source
            .into_iter()
            .map(|id| ("value".into(), id, Ty::Str))
            .collect(),
        result: Ty::Str,
        alarm: false,
        generator: None,
        start: Vec::new(),
        global_state: false,
        globals: Vec::new(),
        configuration: Vec::new(),
        stop: Vec::new(),
        caches: Vec::new(),
        handlers: source
            .map(|_| {
                (
                    None,
                    vec![Statement::Return(Value::new(
                        Ty::Str,
                        Kind::Binary("+".into(), Box::new(value.clone()), Box::new(value)),
                    ))],
                )
            })
            .into_iter()
            .collect(),
    }
}
#[test]
fn branches_use_maximum_but_chains_accumulate_width() {
    for branches in [2, 16] {
        let mut plan = Plan {
            nodes: vec![node(None)],
            ..Plan::default()
        };
        plan.nodes.extend((0..branches).map(|_| node(Some(0))));
        assert_eq!(hgl_rust::pure_bounds::text_factor(&plan), 2);
    }
    let mut chain = Plan {
        nodes: vec![node(None)],
        ..Plan::default()
    };
    for previous in 0..5 {
        chain.nodes.push(node(Some(previous)));
    }
    assert_eq!(hgl_rust::pure_bounds::text_factor(&chain), 32);
}

fn concat(value: Value) -> Value {
    Value::new(
        Ty::Str,
        Kind::Binary("+".into(), Box::new(value.clone()), Box::new(value)),
    )
}
#[test]
fn nested_helpers_inherit_argument_field_bounds_without_caller_local_aliases() {
    let pair = Ty::Tuple(vec![Ty::Str, Ty::Str]);
    for (field, expected) in [(0, 8), (1, 4)] {
        let input = Value::new(Ty::Str, Kind::Input(0, false));
        let parameter = Value::new(
            Ty::Str,
            Kind::Field(Box::new(Value::new(pair.clone(), Kind::Local(0))), field),
        );
        let inner_text = concat(Value::new(Ty::Str, Kind::Local(0)));
        let inner = Value::new(
            Ty::Bool,
            Kind::ValueCall(
                vec![Value::new(Ty::Str, Kind::Local(1))],
                vec![Statement::Return(Value::new(
                    Ty::Bool,
                    Kind::Binary(
                        "==".into(),
                        Box::new(inner_text.clone()),
                        Box::new(inner_text),
                    ),
                ))],
            ),
        );
        let outer = Value::new(
            Ty::Bool,
            Kind::ValueCall(
                vec![Value::new(pair.clone(), Kind::Local(1))],
                vec![
                    Statement::Let(1, concat(parameter)),
                    Statement::Return(inner),
                ],
            ),
        );
        let mut consumer = node(Some(0));
        consumer.result = Ty::Bool;
        consumer.handlers[0].1 = vec![
            Statement::Let(0, concat(input.clone())),
            Statement::Let(
                1,
                Value::new(
                    pair.clone(),
                    Kind::Construct(vec![(0, Value::new(Ty::Str, Kind::Local(0))), (1, input)]),
                ),
            ),
            Statement::Return(outer),
        ];
        let mut plan = Plan {
            nodes: vec![node(None), consumer],
            ..Plan::default()
        };
        assert_eq!(hgl_rust::pure_bounds::text_factor(&plan), expected);
        hgl_semantics::call_locals::prepare(&mut plan);
        assert_eq!(hgl_rust::pure_bounds::text_factor(&plan), expected);
    }
}
