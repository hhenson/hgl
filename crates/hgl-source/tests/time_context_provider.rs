//! Exact provider membership and retained identity precede graph execution.
use hgl_source::literals::{Literal, TemporalLiteral};
use hgl_source::time_context::RunContext;

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn zoned(zone: &str, instant_micros: i64, offset_seconds: i32) -> TemporalLiteral {
    TemporalLiteral::ZonedDateTime {
        instant_micros,
        zone: zone.into(),
        offset_seconds,
    }
}
#[test]
fn catalog_case_aliases_offsets_and_ranges_are_strict() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = RunContext::from_bundled()?;
    let mut retained = Vec::new();
    for name in ["UTC", "Etc/UTC", "America/New_York", "US/Eastern"] {
        let value = context.materialize(&TemporalLiteral::TimeZone(name.into()))?;
        let Literal::TimeZone(zone) = &value else {
            panic!("expected timezone")
        };
        assert_eq!(zone.as_str(), name);
        retained.push(value);
    }
    assert_ne!(retained[0], retained[1]);
    assert_ne!(retained[2], retained[3]);
    for name in ["utc", "america/new_york", "Etc/Unknown", "Missing/Zone"] {
        for recipe in [TemporalLiteral::TimeZone(name.into()), zoned(name, 0, 0)] {
            let error = context.materialize(&recipe).unwrap_err();
            assert!(
                error.contains("absent from exact provider catalog"),
                "{error}"
            );
        }
    }
    let eastern = context.materialize(&zoned("US/Eastern", 0, -18_000))?;
    let utc = context.materialize(&zoned("UTC", 0, 0))?;
    assert_ne!(eastern, utc);
    assert!(
        context
            .materialize(&zoned("US/Eastern", 0, -14_400))
            .unwrap_err()
            .contains("offset mismatch")
    );
    context.materialize(&zoned("US/Eastern", 1_593_561_600_000_000, -14_400))?;
    assert!(
        context
            .materialize(&zoned("US/Eastern", 1_593_561_600_000_000, -18_000))
            .unwrap_err()
            .contains("offset mismatch")
    );
    assert!(
        context
            .materialize(&zoned("UTC", i64::MAX, 0))
            .unwrap_err()
            .contains("range")
    );
    drop(context);
    let Literal::ZonedDateTime(value) = eastern else {
        panic!("expected zoned datetime")
    };
    assert_eq!(value.instant().micros(), 0);
    assert_eq!(value.zone().as_str(), "US/Eastern");
    assert_eq!(value.offset_seconds(), -18_000);
    Ok(())
}
#[cfg(unix)]
#[test]
fn configured_catalog_has_no_synthetic_names_and_is_snapshotted()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "hgl-zone-catalog-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos(),
        NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root)?;
    assert!(
        RunContext::from_database(jiff::tz::TimeZoneDatabase::none())
            .unwrap_err()
            .contains("catalog is empty")
    );
    let source = std::path::Path::new("/usr/share/zoneinfo/Etc/UTC");
    std::fs::copy(source, root.join("ExactAlias"))?;
    let database = jiff::tz::TimeZoneDatabase::from_dir(&root)?;
    let mut context = RunContext::from_database(database)?;
    std::fs::copy(source, root.join("LaterAlias"))?;
    for name in ["UTC", "Etc/Unknown", "exactalias", "LaterAlias"] {
        assert!(
            context
                .materialize(&TemporalLiteral::TimeZone(name.into()))
                .unwrap_err()
                .contains("absent from exact provider catalog")
        );
    }
    let saved = context.materialize(&TemporalLiteral::TimeZone("ExactAlias".into()))?;
    let saved_clock = context.materialize(&TemporalLiteral::ZonedTime {
        time_micros: 34_200_123_456,
        zone: "ExactAlias".into(),
    })?;
    drop(context);
    std::fs::remove_dir_all(root)?;
    let Literal::ZonedTime(clock) = saved_clock else {
        panic!("zoned time")
    };
    assert_eq!(clock.time().0, 34_200_123_456);
    assert_eq!(clock.zone().as_str(), "ExactAlias");
    let Literal::TimeZone(zone) = saved else {
        panic!("expected timezone")
    };
    assert_eq!(zone.as_str(), "ExactAlias");
    Ok(())
}

#[test]
fn zoned_time_validates_catalog_and_wall_range_without_date_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let mut context = RunContext::from_bundled()?;
    let recipe = |time_micros, zone: &str| TemporalLiteral::ZonedTime {
        time_micros,
        zone: zone.into(),
    };
    let original = context.materialize(&recipe(34_200_123_456, "US/Eastern"))?;
    assert_ne!(
        original,
        context.materialize(&recipe(34_200_123_456, "America/New_York"))?
    );
    for name in ["utc", "america/new_york", "Etc/Unknown", "Missing/Zone"] {
        assert!(
            context
                .materialize(&recipe(0, name))
                .unwrap_err()
                .contains("absent from exact provider catalog")
        );
    }
    for time in [-1, 86_400_000_000, i64::MAX] {
        assert!(
            context
                .materialize(&recipe(time, "UTC"))
                .unwrap_err()
                .contains("zoned time range")
        );
    }
    context.materialize(&recipe(0, "UTC"))?;
    context.materialize(&recipe(86_399_999_999, "UTC"))?;
    drop(context);
    let Literal::ZonedTime(value) = original else {
        panic!("zoned time")
    };
    assert_eq!(value.time().0, 34_200_123_456);
    assert_eq!(value.zone().as_str(), "US/Eastern");
    Ok(())
}
