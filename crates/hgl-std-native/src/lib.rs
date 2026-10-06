//! Rust implementations of the standard library's native scalar interfaces.
pub mod eval_buffers;
use hgl_kernel::NodeError;
use hgl_types::{Date, EngineDelta, EngineTime, Time};
type Result<T> = std::result::Result<T, Box<NodeError>>;
/// Unicode scalar count.
pub fn len_str(value: &str) -> i64 {
    i64::try_from(value.chars().count()).unwrap_or(i64::MAX)
}
/// Empty text.
pub fn is_empty_str(value: &str) -> bool {
    value.is_empty()
}
/// Substring membership.
pub fn contains_str_str(value: &str, needle: &str) -> bool {
    value.contains(needle)
}
/// Text prefix.
pub fn starts_with_str_str(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
}
/// Text suffix.
pub fn ends_with_str_str(value: &str, suffix: &str) -> bool {
    value.ends_with(suffix)
}
/// Floating truth value.
pub fn truthy_f64(value: f64) -> bool {
    value != 0.0
}
/// Text truth value.
pub fn truthy_str(value: &str) -> bool {
    !value.is_empty()
}
/// Integer bit intersection.
pub fn bit_and_i64_i64(lhs: i64, rhs: i64) -> i64 {
    lhs & rhs
}
/// Checked integral exponentiation.
pub fn power_i64_i64(lhs: i64, rhs: i64) -> Result<i64> {
    let rhs = u32::try_from(rhs)
        .map_err(|e| NodeError::new(format!("integer power exponent out of range: {e}")))?;
    lhs.checked_pow(rhs)
        .ok_or_else(|| NodeError::new("integer power overflow"))
}
/// Floating exponentiation rejects zero to a negative power.
pub fn power_f64_f64(lhs: f64, rhs: f64) -> Result<f64> {
    if lhs == 0.0 && rhs < 0.0 {
        Err(NodeError::new("zero to negative power"))
    } else {
        Ok(lhs.powf(rhs))
    }
}
/// Checked arithmetic left shift.
pub fn shift_left_i64_i64(lhs: i64, rhs: i64) -> Result<i64> {
    if rhs < 0 {
        return Err(NodeError::new("negative shift count"));
    }
    if lhs == 0 {
        return Ok(0);
    }
    let count = u32::try_from(rhs).map_err(|e| NodeError::new(format!("shift overflow: {e}")))?;
    if count > 63 {
        return Err(NodeError::new("shift overflow"));
    }
    i64::try_from(i128::from(lhs) << count)
        .map_err(|e| NodeError::new(format!("shift overflow: {e}")))
}
/// Arithmetic right shift with Python's large-count behaviour.
pub fn shift_right_i64_i64(lhs: i64, rhs: i64) -> Result<i64> {
    if rhs < 0 {
        return Err(NodeError::new("negative shift count"));
    }
    Ok(if rhs >= 64 {
        if lhs < 0 { -1 } else { 0 }
    } else {
        lhs >> rhs
    })
}
/// Python slice bounds measured in Unicode scalar values.
pub fn slice_str_i64_i64(value: &str, begin: i64, end: i64) -> String {
    let length = i64::try_from(value.chars().count()).unwrap_or(i64::MAX);
    let bound = |n: i64| {
        if n < 0 {
            length.saturating_add(n).max(0)
        } else {
            n.min(length)
        }
    };
    value
        .chars()
        .enumerate()
        .filter_map(|(i, c)| {
            let i = i64::try_from(i).ok()?;
            (bound(begin) <= i && i < bound(end)).then_some(c)
        })
        .collect()
}
/// Round the original binary float, ties to even.
pub fn round_decimal_f64_i64(value: f64, digits: i64) -> Result<f64> {
    if digits > 308 {
        return Ok(value);
    }
    if digits < -308 {
        return Ok(0.0_f64.copysign(value));
    }
    if digits < 0 {
        let scale =
            10.0_f64.powi(i32::try_from(-digits).map_err(|e| NodeError::new(e.to_string()))?);
        return Ok((value / scale).round_ties_even() * scale);
    }
    let precision = usize::try_from(digits).map_err(|e| NodeError::new(e.to_string()))?;
    format!("{value:.precision$}")
        .parse::<f64>()
        .map_err(|e| NodeError::new(e.to_string()))
}
/// Python boolean text.
pub fn as_str_bool(value: bool) -> String {
    if value { "True" } else { "False" }.into()
}
/// Integer text.
pub fn as_str_i64(value: i64) -> String {
    value.to_string()
}
/// Python-style finite scalar float text.
pub fn as_str_f64(value: f64) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    let text = format!("{value:?}");
    if let Some((mantissa, exponent)) = text.split_once('e') {
        let (sign, digits) = exponent
            .strip_prefix('-')
            .map_or(("+", exponent), |digits| ("-", digits));
        format!("{mantissa}e{sign}{digits:0>2}")
    } else {
        text
    }
}
/// Text identity.
pub fn as_str_str(value: &str) -> String {
    value.to_owned()
}
/// Date text.
pub fn as_str_date(value: Date) -> String {
    hgl_calendar::date_text(value)
}
/// Clock text.
pub fn as_str_time(value: Time) -> String {
    hgl_calendar::time_text(value)
}
/// UTC instant text.
pub fn as_str_datetime(value: EngineTime) -> String {
    hgl_calendar::datetime_text(value)
}
/// Normalized interval text.
pub fn as_str_duration(value: EngineDelta) -> String {
    hgl_calendar::duration_text(value)
}
/// Calendar year.
pub fn year_date(value: Date) -> i64 {
    hgl_calendar::components(value).0
}
/// Calendar month.
pub fn month_date(value: Date) -> i64 {
    hgl_calendar::components(value).1
}
/// Calendar day.
pub fn day_date(value: Date) -> i64 {
    hgl_calendar::components(value).2
}
/// UTC midnight at the start of an admitted calendar date.
pub fn midnight_date(value: Date) -> EngineTime {
    EngineTime::from_micros(value.0 * hgl_calendar::DAY)
}
/// UTC calendar year.
pub fn year_datetime(value: EngineTime) -> i64 {
    year_date(Date(value.micros().div_euclid(hgl_calendar::DAY)))
}
/// UTC hour.
pub fn hour_datetime(value: EngineTime) -> i64 {
    value.micros().rem_euclid(hgl_calendar::DAY) / 3_600_000_000
}
/// Clock hour.
pub fn hour_time(value: Time) -> i64 {
    value.0 / 3_600_000_000
}
/// Normalized whole days.
pub fn days_duration(value: EngineDelta) -> i64 {
    value.micros().div_euclid(hgl_calendar::DAY)
}
/// Normalized seconds within the day.
pub fn seconds_duration(value: EngineDelta) -> i64 {
    value.micros().rem_euclid(hgl_calendar::DAY) / 1_000_000
}
/// Normalized microsecond remainder.
pub fn microseconds_duration(value: EngineDelta) -> i64 {
    value.micros().rem_euclid(1_000_000)
}
/// Epoch seconds.
#[expect(
    clippy::cast_precision_loss,
    reason = "floating timestamp follows the scalar native contract"
)]
pub fn timestamp_datetime(value: EngineTime) -> f64 {
    value.micros() as f64 / 1_000_000.0
}
/// Integer-to-float projection.
#[expect(
    clippy::cast_precision_loss,
    reason = "the native conversion explicitly targets f64"
)]
pub fn as_float_i64(value: i64) -> f64 {
    value as f64
}
/// Truncate a float toward zero.
#[expect(
    clippy::cast_possible_truncation,
    reason = "native conversion explicitly truncates toward zero"
)]
pub fn as_int_f64(value: f64) -> i64 {
    value as i64
}
/// Print the diagnostic text supplied by HGL.
#[expect(clippy::print_stdout, reason = "this is the native diagnostic sink")]
pub fn print_line_str(text: &str) {
    println!("{text}");
}
/// Integral addition.
pub fn add_i64_i64(lhs: i64, rhs: i64) -> i64 {
    lhs.wrapping_add(rhs)
}
/// Floating addition.
pub fn add_f64_f64(lhs: f64, rhs: f64) -> f64 {
    lhs + rhs
}
/// Mixed addition.
pub fn add_i64_f64(lhs: i64, rhs: f64) -> f64 {
    as_float_i64(lhs) + rhs
}
/// Mixed addition.
pub fn add_f64_i64(lhs: f64, rhs: i64) -> f64 {
    lhs + as_float_i64(rhs)
}
/// Concatenate text.
pub fn add_str_str(lhs: &str, rhs: &str) -> Result<String> {
    let mut out = String::new();
    out.try_reserve(
        lhs.len()
            .checked_add(rhs.len())
            .ok_or_else(|| NodeError::new("text length overflow"))?,
    )
    .map_err(|e| NodeError::new(e.to_string()))?;
    out.push_str(lhs);
    out.push_str(rhs);
    Ok(out)
}
/// True division of integral values.
pub fn div_i64_i64(lhs: i64, rhs: i64) -> Result<f64> {
    if rhs == 0 {
        Err(NodeError::new("division by zero"))
    } else {
        Ok(as_float_i64(lhs) / as_float_i64(rhs))
    }
}
/// Floor division, including negative divisors.
pub fn floordiv_i64_i64(lhs: i64, rhs: i64) -> Result<i64> {
    let quotient = lhs
        .checked_div(rhs)
        .ok_or_else(|| NodeError::new("invalid integer division"))?;
    let remainder = lhs
        .checked_rem(rhs)
        .ok_or_else(|| NodeError::new("invalid integer division"))?;
    Ok(if remainder != 0 && (remainder < 0) != (rhs < 0) {
        quotient - 1
    } else {
        quotient
    })
}
/// Modulo with the sign of the divisor.
pub fn mod_i64_i64(lhs: i64, rhs: i64) -> Result<i64> {
    if rhs == -1 {
        return Ok(0);
    }
    let remainder = lhs
        .checked_rem(rhs)
        .ok_or_else(|| NodeError::new("modulo by zero"))?;
    Ok(if remainder != 0 && (remainder < 0) != (rhs < 0) {
        remainder + rhs
    } else {
        remainder
    })
}
/// Informational logger service selected by the Rust test host.
#[expect(
    clippy::print_stderr,
    reason = "native logger service writes to the host diagnostic stream"
)]
pub fn log_info_str(text: &str) {
    eprintln!("INFO {text}");
}
/// Raise the HGL assertion failure through the node error channel.
pub fn raise_error_str(text: &str) -> Result<()> {
    Err(NodeError::new(text))
}
