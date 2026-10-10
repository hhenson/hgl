//! Boxing-only ownership lowering cannot replace ordinary reads or helper captures.
use hgl_semantics::{
    boxed_uses,
    ir::{Kind, Statement, Value},
};
use hgl_source::Ty;
fn local(id: usize) -> Value {
    Value::new(Ty::I64, Kind::Local(id))
}
fn boxed(id: usize) -> Value {
    Value::new(Ty::Any, Kind::Unary("any".into(), Box::new(local(id))))
}
#[test]
fn exclusive_boxing_keeps_ordinary_and_helper_consumers_distinct() {
    let binding = Statement::Let(
        0,
        Value::new(Ty::I64, Kind::Literal(hgl_source::Literal::Int(7))),
    );
    let selected = |body: &[Statement]| boxed_uses::exclusive(body, |_| true);
    assert_eq!(
        selected(&[binding.clone(), Statement::Return(boxed(0))])
            .into_iter()
            .collect::<Vec<_>>(),
        [0]
    );
    assert!(
        selected(&[
            binding.clone(),
            Statement::Let(1, local(0)),
            Statement::Return(boxed(0))
        ])
        .is_empty()
    );
    assert!(
        selected(&[
            binding.clone(),
            Statement::Call(Value::new(
                Ty::I64,
                Kind::ValueCall(vec![local(0)], vec![Statement::Return(local(0))])
            )),
            Statement::Return(boxed(0))
        ])
        .is_empty()
    );
    let helper = Value::new(
        Ty::I64,
        Kind::ValueCall(
            vec![Value::new(
                Ty::I64,
                Kind::Literal(hgl_source::Literal::Int(1)),
            )],
            vec![Statement::Return(local(0))],
        ),
    );
    assert_eq!(
        selected(&[
            binding,
            Statement::Call(helper),
            Statement::Return(boxed(0))
        ])
        .into_iter()
        .collect::<Vec<_>>(),
        [0]
    );
}
