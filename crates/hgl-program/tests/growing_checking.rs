//! Growing publication formation and pre-start trace canonicality.
use hgl_rust::{DeltaEntry, Kind, Value};
use hgl_source::{Literal, Ty};
fn delta(items: &[i64], removed: &[i64]) -> Value {
    Value::new(
        Ty::Delta(Box::new(Ty::List(Box::new(Ty::I64), None))),
        Kind::Delta(
            items
                .iter()
                .map(|index| {
                    DeltaEntry::Child(*index, Value::new(Ty::I64, Kind::Literal(Literal::Int(1))))
                })
                .chain(removed.iter().map(|index| {
                    DeltaEntry::Remove(Value::new(Ty::I64, Kind::Literal(Literal::Int(*index))))
                }))
                .collect(),
        ),
    )
}
#[test]
fn trace_validation_reports_first_bad_publication() {
    let shape = Ty::List(Box::new(Ty::I64), None);
    for bad in [
        delta(&[3], &[]),
        delta(&[], &[0]),
        delta(&[], &[2]),
        delta(&[1], &[1]),
        delta(&[], &[]),
    ] {
        let (index, _) = hgl_semantics::eval_data::validate(
            &shape,
            &[Some(delta(&[0, 1], &[])), None, Some(bad)],
        )
        .unwrap_err();
        assert_eq!(index, 2);
    }
    assert!(
        hgl_semantics::eval_data::validate(
            &shape,
            &[
                Some(delta(&[0, 1], &[])),
                Some(delta(&[], &[1, 0])),
                Some(delta(&[0], &[]))
            ]
        )
        .is_ok()
    );
}
#[test]
fn construction_checks_indices_and_keeps_empty_ordinary_values() {
    let check = |value: &str| {
        hgl_program::compile_tests(&[(
            "growing.hgl".into(),
            format!("module growing\ntest data {{let value={value}}}"),
        )])
    };
    check("delta<list<i64>>()").unwrap();
    check("delta<list<i64>>(items:[4:1])").unwrap();
    check("delta<list<i64>>(items:[0:1],remove:[0])").unwrap();
    let shape = Ty::List(Box::new(Ty::I64), None);
    assert_eq!(
        hgl_semantics::eval_data::validate(&shape, &[Some(delta(&[0], &[0]))])
            .unwrap_err()
            .0,
        0
    );
    for (value, message) in [
        ("delta<list<i64>>(items:[-1:1])", "delta.index_bounds"),
        ("delta<list<i64>>(remove:[-1])", "delta.index_bounds"),
        ("delta<list<i64>>(items:[0:1,0:2])", "delta.duplicate_entry"),
        ("delta<list<i64>>(remove:[0,0])", "delta.duplicate_entry"),
        ("delta<list<i64>>(remove:[0.0])", "delta.entry_type"),
        ("delta<list<i64,2>>(remove:[1])", "delta.argument_name"),
    ] {
        let error = check(value).unwrap_err();
        assert!(error.contains(message), "{value}: {error}");
    }
}
