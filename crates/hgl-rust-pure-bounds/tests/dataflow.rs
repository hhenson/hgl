//! Independent branches must not multiply one another's required text width.
use hgl_rust_ir::{Kind, Node, Plan, Statement, Value};
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
        assert_eq!(hgl_rust_pure_bounds::text_factor(&plan), 2);
    }
    let mut chain = Plan {
        nodes: vec![node(None)],
        ..Plan::default()
    };
    for previous in 0..5 {
        chain.nodes.push(node(Some(previous)));
    }
    assert_eq!(hgl_rust_pure_bounds::text_factor(&chain), 32);
}
