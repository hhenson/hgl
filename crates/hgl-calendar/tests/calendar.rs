//! Gregorian boundaries and normalized negative intervals.
use hgl_calendar::{
    date, date_text, datetime, datetime_text, duration, duration_text, time, time_text,
};
use hgl_types::EngineDelta;
#[test]
fn calendar_round_trips_and_rejects_invalid_dates() {
    for value in [
        "0001-01-01",
        "1900-02-28",
        "1969-12-31",
        "1970-01-01",
        "2000-02-29",
        "2024-02-29",
        "9999-12-31",
    ] {
        assert_eq!(date_text(date(value).unwrap()), value);
    }
    for value in [
        "0000-01-01",
        "1900-02-29",
        "2023-02-29",
        "2024-13-01",
        "2024-00-01",
        "2024-04-31",
    ] {
        assert!(date(value).is_err(), "{value}");
    }
    assert_eq!(date("1970-01-01").unwrap().0, 0);
    assert_eq!(
        datetime_text(datetime("1969-12-31T23:59:59.999999Z").unwrap()),
        "1969-12-31 23:59:59.999999"
    );
}
#[test]
fn clock_precision_and_compound_duration_are_checked() {
    assert_eq!(time_text(time("09:30:15.25").unwrap()), "09:30:15.250000");
    for value in ["24:00", "23:60", "00:00:60", "00:00:01.1234567"] {
        assert!(time(value).is_err());
    }
    assert_eq!(duration("1d1h30m1s2us").unwrap().micros(), 91_801_000_002);
    assert!(duration("9223372036854775807d").is_err());
    assert_eq!(
        duration_text(EngineDelta::from_micros(-1)),
        "-1 day, 23:59:59.999999"
    );
}
