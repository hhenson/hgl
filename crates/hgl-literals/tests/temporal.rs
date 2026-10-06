//! Source recipes preserve spelling and never consult a provider.
use hgl_literals::{Literal, ParsedLiteral, TemporalLiteral, numeric};
use hgl_type_shape::Ty;

fn parse(source: &str) -> Result<ParsedLiteral, String> {
    numeric(source, false)?.ok_or_else(|| "expected literal".into())
}
#[test]
fn calendar_categories_and_explicit_offsets_are_exact() -> Result<(), String> {
    assert!(matches!(
        parse("@2024-02-29T12:30:00.123456")?,
        ParsedLiteral::Value(Literal::CivilDateTime(_))
    ));
    assert_eq!(
        parse("@2026-01-15T13:30+01:00")?,
        parse("@2026-01-15T12:30Z")?
    );
    assert_eq!(parse("@2026-01-15T13:30+01")?, parse("@2026-01-15T12:30Z")?);
    let ParsedLiteral::Contextual(TemporalLiteral::ZonedDateTime {
        instant_micros,
        zone,
        offset_seconds,
    }) = parse("@2026-01-15T13:30+01:00[Europe/Paris]")?
    else {
        panic!("contextual recipe")
    };
    assert_eq!(zone, "Europe/Paris");
    assert_eq!(offset_seconds, 3600);
    assert_eq!(
        ParsedLiteral::Value(Literal::DateTime(instant_micros)),
        parse("@2026-01-15T12:30Z")?
    );
    Ok(())
}
#[test]
fn catalog_membership_is_deferred_and_exact_names_are_retained() -> Result<(), String> {
    for name in [
        "UTC",
        "Etc/UTC",
        "US/Eastern",
        "America/New_York",
        "america/new_york",
        "NoSuch/Zone",
    ] {
        assert_eq!(
            parse(&format!("@[{name}]"))?,
            ParsedLiteral::Contextual(TemporalLiteral::TimeZone(name.into()))
        );
    }
    for text in [
        "@[]",
        "@[/UTC]",
        "@[UTC/]",
        "@[a//b]",
        "@[a/../b]",
        "@[a/./b]",
        "@[a b]",
        "@[é]",
        "@[UTC",
    ] {
        assert!(numeric(text, false).is_err(), "{text}");
    }
    assert!(numeric(&format!("@[{}]", "a".repeat(256)), false).is_err());
    Ok(())
}
#[test]
fn invalid_widths_ranges_and_offset_bearing_zoned_time_are_rejected() {
    for text in [
        "@2024-2-29",
        "@0000-01-01",
        "@2023-02-29",
        "@1:30",
        "@24:00",
        "@12:30.1",
        "@12:30:00.",
        "@12:30:00.1234567",
        "@2026-01-15T12:30+1",
        "@2026-01-15T12:30+24:00",
        "@0001-01-01T00:00+01",
        "@9999-12-31T23:59-01",
        "@12:30+01[Europe/Paris]",
        "@2026-01-15T12:30[Europe/Paris]",
    ] {
        assert!(numeric(text, false).is_err(), "{text}");
    }
}
#[test]
fn new_leaves_normalize_atomic_and_delta_without_erasing_composite_identity() {
    for name in ["civil_datetime", "timezone", "zoned_datetime", "zoned_time"] {
        let scalar = Ty::parse(name).unwrap();
        assert_eq!(Ty::parse(&format!("atomic<{name}>")), Some(scalar.clone()));
        assert_eq!(scalar.clone().delta().unwrap(), scalar);
        assert!(scalar.atomic_payload());
        assert!(
            Ty::parse(&format!("atomic<list<{name}>>"))
                .unwrap()
                .publication()
        );
    }
    assert_eq!(Ty::parse("zoned_time"), Some(Ty::ZonedTime));
}

#[test]
fn zoned_times_preserve_wall_microseconds_and_defer_catalog_membership() -> Result<(), String> {
    for (clock, micros) in [
        ("00:00", 0),
        ("09:30:00.123456", 34_200_123_456),
        ("23:59:59.999999", 86_399_999_999),
    ] {
        for zone in [
            "UTC",
            "US/Eastern",
            "America/New_York",
            "Missing/Zone",
            "utc",
        ] {
            assert_eq!(
                parse(&format!("@{clock}[{zone}]"))?,
                ParsedLiteral::Contextual(TemporalLiteral::ZonedTime {
                    time_micros: micros,
                    zone: zone.into()
                })
            );
        }
    }
    for clock in [
        "24:00",
        "23:59:60",
        "1:30",
        "12:30.1",
        "12:30:00.1234567",
        "12:30Z",
        "12:30+01",
        "2026-01-15",
    ] {
        assert!(parse(&format!("@{clock}[UTC]")).is_err(), "{clock}");
    }
    assert!(numeric("@09:30[UTC]", true).is_err());
    Ok(())
}
