//! Duplicate keys stop before their values and later source expressions.
use hgl_rust_ir::{Kind, Value};
use hgl_source::{Literal, TemporalLiteral, Ty};
use hgl_value_eval::{EvalError, Evaluator};
fn zone(name: &str) -> Value {
    Value::new(
        Ty::TimeZone,
        Kind::TemporalLiteral(TemporalLiteral::TimeZone(name.into())),
    )
}
fn map(entries: &[(&str, &str)]) -> Value {
    Value::new(
        Ty::Map(Box::new(Ty::TimeZone), Box::new(Ty::TimeZone)),
        Kind::List(
            entries
                .iter()
                .map(|(k, v)| {
                    Value::new(
                        Ty::Tuple(vec![Ty::TimeZone, Ty::TimeZone]),
                        Kind::Construct(vec![(0, zone(k)), (1, zone(v))]),
                    )
                })
                .collect(),
        ),
    )
}
#[test]
fn map_retains_once_in_order_and_duplicate_precedes_value() {
    for (entries, expected, failed) in [
        (
            vec![("a", "first"), ("b", "second")],
            vec!["a", "first", "b", "second"],
            false,
        ),
        (
            vec![("a", "first"), ("a", "unreached"), ("b", "later")],
            vec!["a", "first", "a"],
            true,
        ),
    ] {
        let mut order = Vec::new();
        let result = Evaluator::default().value_with(&map(&entries), &mut |recipe| {
            let TemporalLiteral::TimeZone(name) = recipe else {
                return Err(EvalError::Unsupported("zone".into()));
            };
            order.push(name.clone());
            Ok(Literal::TimeZone(
                hgl_time_values::ZoneId::from_validated_name(name.clone()),
            ))
        });
        assert_eq!(order, expected);
        assert_eq!(result.is_err(), failed);
        if let Ok(value) = result {
            assert!(value.closed());
            assert!(Evaluator::default().value(&value).is_ok());
        }
    }
}
#[test]
fn floating_keys_reject_nan_and_normalize_signed_zero() {
    for keys in [vec![0.0, -0.0], vec![f64::NAN]] {
        let value = Value::new(
            Ty::Set(Box::new(Ty::F64)),
            Kind::List(
                keys.into_iter()
                    .map(|v| Value::new(Ty::F64, Kind::Literal(Literal::Float(v))))
                    .collect(),
            ),
        );
        assert!(Evaluator::default().value(&value).is_err());
    }
}
