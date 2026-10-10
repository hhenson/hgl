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
