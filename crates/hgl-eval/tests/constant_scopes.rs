//! Constant classification preserves lexical scope and expression boundaries.
use hgl_eval::ir::{Kind, Statement, Value};
use hgl_eval::value_constant::context_free;
use hgl_value_model::{Literal, Ty};

fn local(id: usize) -> Value {
    Value::new(Ty::I64, Kind::Local(id))
}
fn literal() -> Value {
    Value::new(Ty::I64, Kind::Literal(Literal::Int(1)))
}
fn call(args: Vec<Value>, body: Vec<Statement>) -> Value {
    Value::new(Ty::I64, Kind::ValueCall(args, body))
}
#[test]
fn helper_arguments_use_caller_scope_before_fresh_body_bindings() {
    let inner = call(vec![local(7)], vec![Statement::Yield(local(0))]);
    let outer = call(
        vec![],
        vec![Statement::Let(7, literal()), Statement::Yield(inner)],
    );
    assert!(context_free(&outer));
    assert!(!outer.closed());
    let escaped = call(
        vec![],
        vec![
            Statement::Let(7, literal()),
            Statement::Yield(call(vec![literal()], vec![Statement::Yield(local(7))])),
        ],
    );
    assert!(!context_free(&escaped));
    assert!(!context_free(&call(
        vec![local(0)],
        vec![Statement::Yield(local(0))],
    )));
}
#[test]
fn constructor_and_context_free_boundaries_remain_distinct() {
    let captured = Value::new(
        Ty::List(Box::new(Ty::I64), None),
        Kind::Captured(9, vec![(8, literal())]),
    );
    assert!(captured.closed());
    assert!(!context_free(&captured));
    let unary = Value::new(Ty::I64, Kind::Unary("-".into(), Box::new(literal())));
    assert!(!unary.closed());
    assert!(context_free(&unary));
    let effect = Value::new(Ty::I64, Kind::GlobalGet(0));
    assert!(!effect.closed());
    assert!(!context_free(&effect));
}
