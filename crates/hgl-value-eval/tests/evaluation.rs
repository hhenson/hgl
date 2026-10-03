//! Independent ordinary evaluation, ownership and failure boundaries.
use hgl_rust_ir::{Kind, Statement, Value};
use hgl_source::{Literal, Ty};
use hgl_value_eval::{EvalError, Evaluator, constant};

fn int(n: i64) -> Value {
    Value::new(Ty::I64, Kind::Literal(Literal::Int(n)))
}
fn bool_value(b: bool) -> Value {
    Value::new(Ty::Bool, Kind::Literal(Literal::Bool(b)))
}
fn list(items: Vec<Value>, element: Ty) -> Value {
    Value::new(Ty::List(Box::new(element), None), Kind::List(items))
}
fn local(id: usize, ty: &Ty, writable: bool) -> Value {
    Value::new(
        ty.clone(),
        if writable {
            Kind::MutableLocal(id)
        } else {
            Kind::Local(id)
        },
    )
}
fn index(parent: Value, offset: i64, ty: Ty) -> Value {
    Value::new(ty, Kind::Index(Box::new(parent), Box::new(int(offset))))
}
fn field(parent: Value, ty: Ty) -> Value {
    Value::new(ty, Kind::Field(Box::new(parent), 0))
}
fn push(parent: Value, item: Value) -> Statement {
    Statement::Call(Value::new(
        Ty::Void,
        Kind::Push(Box::new(parent), Box::new(item)),
    ))
}
fn binary(op: &str, a: Value, b: Value, ty: Ty) -> Value {
    Value::new(ty, Kind::Binary(op.into(), Box::new(a), Box::new(b)))
}
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Test assertion helper reports evaluation failures directly"
)]
fn number(evaluator: &mut Evaluator, value: &Value) -> i64 {
    let Kind::Literal(Literal::Int(n)) = evaluator.value(value).unwrap().kind else {
        panic!("expected i64")
    };
    n
}
fn size(parent: Value) -> Value {
    Value::new(Ty::I64, Kind::Length(Box::new(parent)))
}

#[test]
fn nested_push_and_owning_assignment_retain_independent_values() {
    let inner = list(vec![int(1)], Ty::I64);
    let outer = list(vec![inner.clone()], inner.ty.clone());
    let mut evaluator = Evaluator::default();
    evaluator
        .statement(&Statement::Var(0, outer.clone()))
        .unwrap();
    evaluator
        .statement(&Statement::Let(1, local(0, &outer.ty, true)))
        .unwrap();
    let writable = local(0, &outer.ty, true);
    evaluator
        .statement(&push(
            writable.clone(),
            index(writable.clone(), 0, inner.ty.clone()),
        ))
        .unwrap();
    evaluator
        .statement(&push(index(writable.clone(), 0, inner.ty.clone()), int(2)))
        .unwrap();
    assert_eq!(
        number(
            &mut evaluator,
            &size(index(writable.clone(), 0, inner.ty.clone()))
        ),
        2
    );
    assert_eq!(
        number(&mut evaluator, &size(index(writable, 1, inner.ty.clone()))),
        1
    );
    assert_eq!(
        number(
            &mut evaluator,
            &size(index(local(1, &outer.ty, false), 0, inner.ty.clone()))
        ),
        1
    );
    evaluator
        .statement(&Statement::Var(2, outer.clone()))
        .unwrap();
    evaluator
        .statement(&Statement::Assign(
            local(2, &outer.ty, true),
            local(0, &outer.ty, true),
        ))
        .unwrap();
    evaluator
        .statement(&push(local(0, &outer.ty, true), inner))
        .unwrap();
    assert_eq!(number(&mut evaluator, &size(local(2, &outer.ty, true))), 2);
}

#[test]
fn fields_beneath_indexed_elements_are_writable_but_not_replaceable() {
    let ty = Ty::Struct("Box".into(), vec![("amount".into(), Ty::I64)]);
    let item = Value::new(ty.clone(), Kind::Construct(vec![(0, int(1))]));
    let items = list(vec![item.clone()], ty.clone());
    let mut evaluator = Evaluator::default();
    evaluator
        .statement(&Statement::Var(0, items.clone()))
        .unwrap();
    let target = index(local(0, &items.ty, true), 0, ty);
    evaluator
        .statement(&Statement::Assign(field(target.clone(), Ty::I64), int(7)))
        .unwrap();
    assert_eq!(number(&mut evaluator, &field(target.clone(), Ty::I64)), 7);
    assert!(matches!(
        evaluator.statement(&Statement::Assign(target, item)),
        Err(EvalError::Unsupported(_))
    ));
}

#[test]
fn branches_scope_bindings_and_keep_outer_writes() {
    let mut evaluator = Evaluator::default();
    evaluator.statement(&Statement::Var(0, int(1))).unwrap();
    evaluator
        .statement(&Statement::If(
            bool_value(true),
            vec![
                Statement::Assign(local(0, &Ty::I64, true), int(2)),
                Statement::Let(0, int(3)),
                Statement::Let(1, int(4)),
            ],
            vec![],
        ))
        .unwrap();
    assert_eq!(number(&mut evaluator, &local(0, &Ty::I64, true)), 2);
    assert!(matches!(
        evaluator.value(&local(1, &Ty::I64, false)),
        Err(EvalError::Unsupported(_))
    ));
}

#[test]
fn direct_calls_use_fresh_readonly_arguments_and_can_copy_to_owners() {
    let items = list(vec![int(1)], Ty::I64);
    let argument = local(0, &items.ty, false);
    let call = Value::new(
        items.ty.clone(),
        Kind::ValueCall(
            vec![items.clone()],
            vec![
                Statement::Var(1, argument),
                push(local(1, &items.ty, true), int(2)),
                Statement::Yield(local(1, &items.ty, true)),
            ],
        ),
    );
    let mut evaluator = Evaluator::default();
    evaluator.statement(&Statement::Var(0, int(99))).unwrap();
    let result = evaluator.value(&call).unwrap();
    assert!(constant(&result));
    assert_eq!(number(&mut evaluator, &size(result)), 2);
    assert_eq!(number(&mut evaluator, &local(0, &Ty::I64, true)), 99);
    let bad_call = Value::new(
        Ty::Void,
        Kind::ValueCall(
            vec![items.clone()],
            vec![push(local(0, &items.ty, true), int(2))],
        ),
    );
    assert!(matches!(
        evaluator.value(&bad_call),
        Err(EvalError::Unsupported(_))
    ));
    assert_eq!(number(&mut evaluator, &size(items)), 1);
}

#[test]
fn bounds_fail_when_evaluated_and_failed_push_preserves_receiver() {
    let items = list(vec![int(1)], Ty::I64);
    let mut evaluator = Evaluator::default();
    evaluator
        .statement(&Statement::Var(0, items.clone()))
        .unwrap();
    for offset in [-1, 1] {
        let missing = index(local(0, &items.ty, true), offset, Ty::I64);
        assert!(matches!(
            evaluator.statement(&push(local(0, &items.ty, true), missing)),
            Err(EvalError::Operation(_))
        ));
    }
    assert_eq!(number(&mut evaluator, &size(local(0, &items.ty, true))), 1);
    let fixed = Value::new(Ty::List(Box::new(Ty::I64), Some(0)), Kind::List(vec![]));
    evaluator
        .statement(&Statement::Var(1, fixed.clone()))
        .unwrap();
    assert!(matches!(
        evaluator.statement(&push(local(1, &fixed.ty, true), int(0))),
        Err(EvalError::Unsupported(_))
    ));
}

#[test]
fn constructors_evaluate_written_field_order_and_retain_children() {
    let items = list(vec![int(1)], Ty::I64);
    let ty = Ty::Struct("Lists".into(), vec![("items".into(), items.ty.clone())]);
    let mut evaluator = Evaluator::default();
    evaluator
        .statement(&Statement::Var(0, items.clone()))
        .unwrap();
    let retained = evaluator
        .value(&Value::new(
            ty,
            Kind::Construct(vec![(0, local(0, &items.ty, true))]),
        ))
        .unwrap();
    evaluator
        .statement(&push(local(0, &items.ty, true), int(2)))
        .unwrap();
    assert_eq!(number(&mut evaluator, &size(field(retained, items.ty))), 1);
    let failure = Value::new(
        Ty::Struct(
            "Pair".into(),
            vec![("a".into(), Ty::I64), ("b".into(), Ty::I64)],
        ),
        Kind::Construct(vec![
            (1, binary("%", int(1), int(0), Ty::I64)),
            (0, index(list(vec![], Ty::I64), 0, Ty::I64)),
        ]),
    );
    assert_eq!(
        evaluator.value(&failure).unwrap_err(),
        EvalError::Operation("division by zero".into())
    );
}

#[test]
fn scalar_operators_obey_floor_modulo_short_circuit_and_failure_boundaries() {
    let mut evaluator = Evaluator::default();
    for (op, a, b, expected) in [
        ("%", -7, 3, 2),
        ("%", 7, -3, -2),
        ("//", -7, 3, -3),
        ("+", 7, 3, 10),
    ] {
        assert_eq!(
            number(&mut evaluator, &binary(op, int(a), int(b), Ty::I64)),
            expected
        );
    }
    let failure = binary("/", int(1), int(0), Ty::F64);
    assert!(matches!(
        evaluator.value(&failure),
        Err(EvalError::Operation(_))
    ));
    let overflow = binary("+", int(i64::MAX), int(1), Ty::I64);
    assert!(matches!(
        evaluator.value(&overflow),
        Err(EvalError::Unsupported(_))
    ));
    let impossible = Value::new(Ty::Bool, Kind::Native(0, vec![]));
    let short = binary("&&", bool_value(false), impossible, Ty::Bool);
    assert!(matches!(
        evaluator.value(&short).unwrap().kind,
        Kind::Literal(Literal::Bool(false))
    ));
}
