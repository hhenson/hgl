//! Prepared constructor recognition must preserve every ordinary call effect.
use hgl_semantics::ir::{Kind, Statement, Value};
use hgl_source::{Literal, Ty};

fn helper(args: Vec<Value>, body: Vec<Statement>) -> Value {
    Value::new(Ty::Bytes, Kind::ValueCall(args, body))
}

#[test]
fn only_the_single_input_effect_free_helper_uses_prepared_constructor_transport() {
    let list = Ty::List(Box::new(Ty::I64), None);
    let input = Value::new(Ty::Atomic(Box::new(list.clone())), Kind::Input(3, false));
    let argument = Value::new(
        list.clone(),
        Kind::Unary("atomic_value".into(), Box::new(input)),
    );
    let result = Value::new(
        Ty::Bytes,
        Kind::Unary("bytes".into(), Box::new(Value::new(list, Kind::Local(0)))),
    );
    let body = vec![Statement::Yield(result)];
    assert_eq!(
        hgl_rust::scalars::bytes_input(&helper(vec![argument.clone()], body.clone())),
        Some(3)
    );
    let effect = Value::new(
        Ty::Void,
        Kind::GlobalSet(
            0,
            Box::new(Value::new(Ty::I64, Kind::Literal(Literal::Int(1)))),
        ),
    );
    assert_eq!(
        hgl_rust::scalars::bytes_input(&helper(
            vec![argument.clone(), effect.clone()],
            body.clone()
        )),
        None
    );
    let mut effectful_body = vec![Statement::Call(effect)];
    effectful_body.extend(body);
    assert_eq!(
        hgl_rust::scalars::bytes_input(&helper(vec![argument], effectful_body)),
        None
    );
}

#[test]
fn ordinary_constructor_fallbacks_never_claim_the_prepared_adapter() {
    use hgl_semantics::ir::{Node, Plan};
    let list = Ty::List(Box::new(Ty::I64), None);
    let mut node = Node {
        name: "bytes-proof".into(),
        inputs: vec![],
        result: Ty::Bytes,
        alarm: false,
        generator: None,
        start: vec![],
        global_state: false,
        globals: vec![],
        configuration: vec![],
        stop: vec![],
        caches: vec![],
        handlers: vec![],
    };
    for kind in [
        Kind::List(vec![Value::new(Ty::I64, Kind::Literal(Literal::Int(1)))]),
        Kind::Local(0),
        Kind::GlobalGet(0),
    ] {
        let construction = Value::new(
            Ty::Bytes,
            Kind::Unary("bytes".into(), Box::new(Value::new(list.clone(), kind))),
        );
        assert_eq!(hgl_rust::scalars::bytes_input(&construction), None);
        node.handlers = vec![(None, vec![Statement::Return(construction)])];
        assert!(!hgl_rust::execution_proof::prepared(&Plan {
            nodes: vec![node.clone()],
            ..Plan::default()
        }));
    }
    let input = Value::new(Ty::Atomic(Box::new(list.clone())), Kind::Input(0, false));
    let construction = Value::new(
        Ty::Bytes,
        Kind::Unary(
            "bytes".into(),
            Box::new(Value::new(
                list,
                Kind::Unary("atomic_value".into(), Box::new(input)),
            )),
        ),
    );
    node.handlers = vec![(None, vec![Statement::Return(construction)])];
    assert!(hgl_rust::execution_proof::prepared(&Plan {
        nodes: vec![node.clone()],
        ..Plan::default()
    }));
    node.result = Ty::Rolling(
        Box::new(Ty::Bytes),
        hgl_source::Window::new(hgl_source::WindowKind::Ticks, 2, 2).unwrap(),
    );
    let return_body = node.handlers[0].1.clone();
    for body in [
        return_body.clone(),
        vec![Statement::If(
            Value::new(Ty::Bool, Kind::Literal(Literal::Bool(true))),
            return_body,
            vec![],
        )],
    ] {
        node.handlers[0].1 = body;
        assert!(hgl_rust::execution_proof::prepared(&Plan {
            nodes: vec![node.clone()],
            ..Plan::default()
        }));
    }
}
