//! Cold keys materialize once, in written order, and validate before replay.
use hgl_semantics::ir::{DeltaEntry, Kind, Value};
use hgl_semantics::value_eval::{EvalError, Evaluator};
use hgl_source::{Literal, TemporalLiteral, Ty};
use hgl_types::time_values::ZoneId;
fn zone(name: &str) -> Value {
    Value::new(
        Ty::TimeZone,
        Kind::TemporalLiteral(TemporalLiteral::TimeZone(name.into())),
    )
}
fn patch(parts: Vec<DeltaEntry>) -> Value {
    Value::new(
        Ty::Delta(Box::new(Ty::Set(Box::new(Ty::TimeZone)))),
        Kind::Delta(parts),
    )
}
#[test]
fn deferred_keys_are_not_closed_and_materialize_once_in_written_order() {
    let value = patch(vec![
        DeltaEntry::Add(zone("UTC")),
        DeltaEntry::Remove(zone("Etc/UTC")),
    ]);
    assert!(!value.closed());
    let mut evaluator = Evaluator::default();
    assert_eq!(
        evaluator.value(&value).unwrap_err(),
        EvalError::ContextRequired
    );
    let mut order = Vec::new();
    let retained = evaluator
        .value_with(&value, &mut |recipe| {
            let TemporalLiteral::TimeZone(name) = recipe else {
                return Err(EvalError::Unsupported("expected zone".into()));
            };
            order.push(name.clone());
            Ok(Literal::TimeZone(ZoneId::from_validated_name(name.clone())))
        })
        .unwrap();
    assert_eq!(order, vec!["UTC", "Etc/UTC"]);
    assert!(retained.closed());
    assert!(
        evaluator
            .value_with(&retained, &mut |_| Err(EvalError::Unsupported(
                "must not rematerialize".into()
            )))
            .is_ok()
    );
}
#[test]
fn duplicate_and_overlap_recipes_fail_after_construction() {
    for parts in [
        vec![DeltaEntry::Add(zone("UTC")), DeltaEntry::Add(zone("UTC"))],
        vec![
            DeltaEntry::Add(zone("UTC")),
            DeltaEntry::Remove(zone("UTC")),
        ],
    ] {
        let mut calls = 0;
        let error = Evaluator::default()
            .value_with(&patch(parts), &mut |_| {
                calls += 1;
                Ok(Literal::TimeZone(ZoneId::from_validated_name("UTC".into())))
            })
            .unwrap_err();
        assert_eq!(calls, 2);
        assert!(
            matches!(error,EvalError::Operation(ref message) if message.contains("duplicate")||message.contains("overlap"))
        );
    }
}
#[test]
fn keyed_payloads_follow_keys_and_failure_stops_later_materialization() {
    let value = Value::new(
        Ty::Delta(Box::new(Ty::Map(
            Box::new(Ty::TimeZone),
            Box::new(Ty::TimeZone),
        ))),
        Kind::Delta(vec![
            DeltaEntry::Keyed(zone("key"), zone("payload")),
            DeltaEntry::Keyed(zone("later"), zone("unreached")),
        ]),
    );
    let mut order = Vec::new();
    let error = Evaluator::default()
        .value_with(&value, &mut |recipe| {
            let TemporalLiteral::TimeZone(name) = recipe else {
                return Err(EvalError::Unsupported("expected zone".into()));
            };
            order.push(name.clone());
            if name == "payload" {
                return Err(EvalError::Operation("invalid payload zone".into()));
            }
            Ok(Literal::TimeZone(ZoneId::from_validated_name(name.clone())))
        })
        .unwrap_err();
    assert_eq!(order, vec!["key", "payload"]);
    assert_eq!(error, EvalError::Operation("invalid payload zone".into()));
}
