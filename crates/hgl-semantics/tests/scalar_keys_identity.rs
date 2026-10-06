//! Cold keys preserve the scalar equality contract without display conversion.
use hgl_semantics::scalar_keys::key;
use hgl_source::{EnumType, Literal};
use hgl_types::time_values::{EngineTime, Time, ZoneId, ZonedDateTime, ZonedTime};

#[test]
fn scalar_type_and_complete_enum_identity_are_part_of_the_key() {
    let enum_type = EnumType {
        origin: "example::E".into(),
        members: vec![("first".into(), 0)],
    };
    let values = vec![
        Literal::Bool(false),
        Literal::Int(0),
        Literal::Float(0.0),
        Literal::Str("0".into()),
        Literal::Date(0),
        Literal::Time(0),
        Literal::DateTime(0),
        Literal::Duration(0),
        Literal::CivilDateTime(0),
        Literal::Enum(enum_type.clone(), 0),
    ];
    let keys = values
        .iter()
        .map(key)
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .unwrap();
    assert_eq!(keys.len(), values.len());
    let other = EnumType {
        origin: "example::Other".into(),
        ..enum_type.clone()
    };
    assert_ne!(
        key(&Literal::Enum(enum_type, 0)).unwrap(),
        key(&Literal::Enum(other, 0)).unwrap()
    );
}
#[test]
fn signed_zeros_are_equal_infinities_distinct_and_nan_rejected() {
    assert_eq!(
        key(&Literal::Float(0.0)).unwrap(),
        key(&Literal::Float(-0.0)).unwrap()
    );
    assert_ne!(
        key(&Literal::Float(f64::INFINITY)).unwrap(),
        key(&Literal::Float(f64::NEG_INFINITY)).unwrap()
    );
    assert!(key(&Literal::Float(f64::NAN)).unwrap_err().contains("NaN"));
}
#[test]
fn zoned_identities_keep_exact_alias_wall_time_instant_and_offset() {
    let utc = ZoneId::from_validated_name("UTC".into());
    let alias = ZoneId::from_validated_name("Etc/UTC".into());
    assert_ne!(
        key(&Literal::TimeZone(utc.clone())).unwrap(),
        key(&Literal::TimeZone(alias.clone())).unwrap()
    );
    assert_ne!(
        key(&Literal::ZonedTime(ZonedTime::from_validated_parts(
            Time(0),
            utc.clone()
        )))
        .unwrap(),
        key(&Literal::ZonedTime(ZonedTime::from_validated_parts(
            Time(0),
            alias.clone()
        )))
        .unwrap()
    );
    let base = Literal::ZonedDateTime(ZonedDateTime::from_validated_parts(
        EngineTime::from_micros(0),
        utc.clone(),
        0,
    ));
    for (instant, zone, offset) in [(1, utc.clone(), 0), (0, alias, 0), (0, utc, 3600)] {
        let other = Literal::ZonedDateTime(ZonedDateTime::from_validated_parts(
            EngineTime::from_micros(instant),
            zone,
            offset,
        ));
        assert_ne!(key(&base).unwrap(), key(&other).unwrap());
    }
}
