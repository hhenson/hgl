//! Composite recipes preserve source order and become independently owned keys.
use hgl_rust_ir::{DeltaEntry, Kind, Statement, Value};
use hgl_source::{Literal, TemporalLiteral, Ty};
use hgl_time_values::ZoneId;
use hgl_value_eval::{EvalError, Evaluator};
fn composite(name: &str) -> Value {
    Value::new(
        Ty::Tuple(vec![Ty::TimeZone, Ty::I64]),
        Kind::Construct(vec![
            (
                0,
                Value::new(
                    Ty::TimeZone,
                    Kind::TemporalLiteral(TemporalLiteral::TimeZone(name.into())),
                ),
            ),
            (1, Value::new(Ty::I64, Kind::Literal(Literal::Int(1)))),
        ]),
    )
}
fn patch(parts: Vec<DeltaEntry>) -> Value {
    Value::new(
        Ty::Delta(Box::new(Ty::Set(Box::new(composite("").ty)))),
        Kind::Delta(parts),
    )
}
#[test]
fn provider_leaves_execute_once_in_order_and_deferred_collisions_fail() -> Result<(), EvalError> {
    let mut evaluator = Evaluator::default();
    let mut order = Vec::new();
    let result = evaluator.value_with(
        &patch(vec![
            DeltaEntry::Add(composite("first")),
            DeltaEntry::Remove(composite("second")),
        ]),
        &mut |recipe| {
            let TemporalLiteral::TimeZone(name) = recipe else {
                return Err(EvalError::ContextRequired);
            };
            order.push(name.clone());
            Ok(Literal::TimeZone(ZoneId::from_validated_name(name.clone())))
        },
    )?;
    assert_eq!(order, ["first", "second"]);
    evaluator.value_with(&result, &mut |_| Err(EvalError::ContextRequired))?;
    for second in [
        DeltaEntry::Add(composite("alias")),
        DeltaEntry::Remove(composite("alias")),
    ] {
        let mut calls = 0;
        let error = evaluator.value_with(
            &patch(vec![DeltaEntry::Add(composite("first")), second]),
            &mut |_| {
                calls += 1;
                Ok(Literal::TimeZone(ZoneId::from_validated_name(
                    "canonical".into(),
                )))
            },
        );
        assert_eq!(calls, 2);
        assert!(matches!(error, Err(EvalError::Operation(_))));
    }
    Ok(())
}
#[test]
fn retaining_delta_key_isolates_later_source_field_writes() -> Result<(), EvalError> {
    let ty = Ty::Struct("keys::Key".into(), vec![("value".into(), Ty::Str)], vec![]);
    let text = |s: &str| Value::new(Ty::Str, Kind::Literal(Literal::Str(s.into())));
    let key = Value::new(ty.clone(), Kind::Construct(vec![(0, text("first"))]));
    let mut evaluator = Evaluator::default();
    evaluator.statement(&Statement::Var(0, key))?;
    let retained = evaluator.value(&Value::new(
        Ty::Delta(Box::new(Ty::Set(Box::new(ty.clone())))),
        Kind::Delta(vec![DeltaEntry::Add(Value::new(
            ty.clone(),
            Kind::MutableLocal(0),
        ))]),
    ))?;
    evaluator.statement(&Statement::Assign(
        Value::new(
            Ty::Str,
            Kind::Field(Box::new(Value::new(ty, Kind::MutableLocal(0))), 0),
        ),
        text("changed"),
    ))?;
    let Kind::Delta(parts) = retained.kind else {
        return Err(EvalError::ContextRequired);
    };
    let DeltaEntry::Add(Value {
        kind: Kind::Construct(fields),
        ..
    }) = &parts[0]
    else {
        return Err(EvalError::ContextRequired);
    };
    assert!(matches!(&fields[0].1.kind,Kind::Literal(Literal::Str(value)) if value=="first"));
    Ok(())
}
