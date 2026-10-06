//! Temporal identity is independent of provider and storage lifetimes.
use hgl_types::time_values::{CivilDateTime, EngineDelta, EngineTime, ZoneId, ZonedDateTime};
#[test]
fn exact_names_and_all_zoned_parts_participate_in_identity()
-> Result<(), std::collections::TryReserveError> {
    let original = ZoneId::from_validated_name("US/Eastern".into());
    let retained = original.try_clone()?;
    assert_eq!(original, retained);
    assert_ne!(original.as_str().as_ptr(), retained.as_str().as_ptr());
    let instant = EngineTime::from_micros(123_456);
    let a = ZonedDateTime::from_validated_parts(instant, original, -18_000);
    let b = ZonedDateTime::from_validated_parts(
        instant,
        ZoneId::from_validated_name("America/New_York".into()),
        -18_000,
    );
    assert_ne!(a, b);
    assert_ne!(
        a,
        ZonedDateTime::from_validated_parts(instant, retained, -14_400)
    );
    let saved = a.try_clone()?;
    assert_ne!(saved.zone().as_str().as_ptr(), a.zone().as_str().as_ptr());
    drop(a);
    assert_eq!(saved.instant(), instant);
    assert_eq!(saved.zone().as_str(), "US/Eastern");
    assert_eq!(saved.offset_seconds(), -18_000);
    assert!(CivilDateTime::from_micros(-1) < CivilDateTime::from_micros(0));
    Ok(())
}
#[test]
fn moved_engine_values_preserve_boundaries_and_result_based_addition() {
    assert!(EngineTime::NEVER < EngineTime::MIN_START);
    assert!(EngineTime::MAX_END < EngineTime::FOREVER);
    assert_eq!(
        EngineTime::MAX_END.checked_add(EngineDelta::STEP),
        Some(EngineTime::FOREVER)
    );
    assert_eq!(EngineTime::FOREVER.checked_add(EngineDelta::STEP), None);
    assert_eq!(
        EngineTime::from_micros(-1).checked_add(EngineDelta::STEP),
        Some(EngineTime::NEVER)
    );
}

#[test]
fn zoned_time_identity_owns_only_wall_time_and_exact_name()
-> Result<(), std::collections::TryReserveError> {
    use hgl_types::time_values::{Time, ZonedTime};
    let original = ZonedTime::from_validated_parts(
        Time(34_200_123_456),
        ZoneId::from_validated_name("US/Eastern".into()),
    );
    let retained = original.try_clone()?;
    assert_eq!(retained, original);
    assert_ne!(
        retained.zone().as_str().as_ptr(),
        original.zone().as_str().as_ptr()
    );
    for (time, zone) in [
        (Time(34_200_123_457), "US/Eastern"),
        (original.time(), "America/New_York"),
    ] {
        assert_ne!(
            original,
            ZonedTime::from_validated_parts(time, ZoneId::from_validated_name(zone.into()))
        );
    }
    drop(original);
    assert_eq!(retained.time(), Time(34_200_123_456));
    assert_eq!(retained.zone().as_str(), "US/Eastern");
    Ok(())
}
