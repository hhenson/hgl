//! Composite proof metadata preserves provider aliases in the executable operand.
use hgl_semantics::ir::{Kind, Value};
use hgl_semantics::static_values::StaticValues;
use hgl_source::{Literal, TemporalLiteral, Ty};
#[test]
fn direct_compound_key_does_not_replay_nested_alias_initializers() -> Result<(), String> {
    let mut scope = StaticValues::default();
    let zone = Value::new(
        Ty::TimeZone,
        Kind::TemporalLiteral(TemporalLiteral::TimeZone("UTC".into())),
    );
    scope.bind(0, &zone, false);
    scope.configuration.push(zone);
    let (configuration, known) = scope.key(Value::new(Ty::TimeZone, Kind::Configuration(0)))?;
    assert!(known.is_none());
    assert!(matches!(configuration.kind, Kind::Configuration(0)));
    let key = Value::new(
        Ty::Tuple(vec![Ty::TimeZone, Ty::I64]),
        Kind::Construct(vec![
            (0, Value::new(Ty::TimeZone, Kind::Local(0))),
            (1, Value::new(Ty::I64, Kind::Literal(Literal::Int(1)))),
        ]),
    );
    let (retained, known) = scope.key(key)?;
    assert!(known.is_none());
    let Kind::Construct(fields) = retained.kind else {
        return Err("complete key required".into());
    };
    assert!(matches!(fields[0].1.kind, Kind::Local(0)));
    Ok(())
}
