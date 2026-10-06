//! Error-channel and boundary cases beyond the shared happy-path suite.
use hgl_stdlib::std_native::{
    add_str_str, floordiv_i64_i64, mod_i64_i64, power_i64_i64, shift_left_i64_i64,
    shift_right_i64_i64, slice_str_i64_i64,
};
#[test]
fn integers_keep_python_floor_and_modulo_signs() {
    for (a, b, q, r) in [
        (-7, 2, -4, 1),
        (7, -2, -4, -1),
        (-7, -2, 3, -1),
        (7, 2, 3, 1),
    ] {
        assert_eq!(floordiv_i64_i64(a, b).unwrap(), q);
        assert_eq!(mod_i64_i64(a, b).unwrap(), r);
    }
    assert_eq!(mod_i64_i64(i64::MIN, -1).unwrap(), 0);
    assert!(floordiv_i64_i64(i64::MIN, -1).is_err());
    assert!(mod_i64_i64(1, 0).is_err());
}
#[test]
fn checked_integer_operations_report_overflow() {
    assert!(power_i64_i64(2, 63).is_err());
    assert!(power_i64_i64(2, -1).is_err());
    assert!(shift_left_i64_i64(1, 63).is_err());
    assert!(shift_left_i64_i64(1, -1).is_err());
    assert_eq!(shift_left_i64_i64(-1, 63).unwrap(), i64::MIN);
    assert_eq!(shift_right_i64_i64(-7, 100).unwrap(), -1);
    assert_eq!(shift_left_i64_i64(0, 100).unwrap(), 0);
}
#[test]
fn text_slicing_counts_unicode_scalars() {
    assert_eq!(slice_str_i64_i64("aé🙂z", 1, -1), "é🙂");
    assert_eq!(slice_str_i64_i64("aé🙂z", -100, 100), "aé🙂z");
    assert_eq!(slice_str_i64_i64("abc", 2, 1), "");
    assert_eq!(add_str_str("é", "🙂").unwrap(), "é🙂");
}

#[test]
fn float_text_keeps_python_exponent_spelling() {
    for (value, text) in [
        (1e-5, "1e-05"),
        (1e16, "1e+16"),
        (-0.0, "-0.0"),
        (f64::NAN, "nan"),
        (f64::INFINITY, "inf"),
    ] {
        assert_eq!(hgl_stdlib::std_native::as_str_f64(value), text);
    }
}

#[test]
fn midnight_preserves_utc_date_at_epoch_leap_day_and_calendar_limits() {
    for day in [
        "0001-01-01",
        "1969-12-31",
        "1970-01-01",
        "2024-02-29",
        "9999-12-31",
    ] {
        let date = hgl_types::calendar::date(day).unwrap();
        let expected = hgl_types::calendar::offset_datetime(&format!("{day}T00:00:00Z"))
            .unwrap()
            .0;
        assert_eq!(hgl_stdlib::std_native::midnight_date(date), expected);
    }
}
