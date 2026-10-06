//! Calendar scalar parsing and projections, independent of graph execution.
use crate::{CivilDateTime, Date, EngineDelta, EngineTime, Time};
/// Microseconds in a day.
pub const DAY: i64 = 86_400_000_000;
fn number(s: &str) -> Result<i64, String> {
    s.parse::<i64>().map_err(|e| e.to_string())
}
fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn start(year: i64) -> i64 {
    let y = year - 1;
    365 * y + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400) - 719_162
}
/// Parse a validated ISO calendar date.
pub fn date(s: &str) -> Result<Date, String> {
    if s.len() != 10
        || !s.bytes().enumerate().all(|(i, c)| {
            if matches!(i, 4 | 7) {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
    {
        return Err("expected YYYY-MM-DD".into());
    }
    let fields = s.split('-').map(number).collect::<Result<Vec<_>, _>>()?;
    let [year, month, day] = fields.as_slice() else {
        return Err("expected YYYY-MM-DD".into());
    };
    let lengths = [
        31,
        if leap(*year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let month = usize::try_from(*month).map_err(|e| e.to_string())?;
    if !(1..=9999).contains(year)
        || !(1..=12).contains(&month)
        || !(1..=lengths[month - 1]).contains(day)
    {
        return Err("invalid calendar date".into());
    }
    Ok(Date(
        start(*year) + lengths[..month - 1].iter().sum::<i64>() + day - 1,
    ))
}
/// Calendar year, month and day of an epoch-relative date.
pub fn components(value: Date) -> (i64, i64, i64) {
    let mut year = 1970 + value.0.div_euclid(365);
    while start(year) > value.0 {
        year -= 1;
    }
    while start(year + 1) <= value.0 {
        year += 1;
    }
    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut day = value.0 - start(year);
    let mut month = 1;
    for length in lengths {
        if day < length {
            break;
        }
        day -= length;
        month += 1;
    }
    (year, month, day + 1)
}
/// Parse an ISO time of day with microsecond precision.
pub fn time(s: &str) -> Result<Time, String> {
    let (clock, fraction) = s
        .split_once('.')
        .map_or((s, None), |(clock, fraction)| (clock, Some(fraction)));
    if !matches!(clock.len(), 5 | 8)
        || !clock.bytes().enumerate().all(|(i, c)| {
            if matches!(i, 2 | 5) {
                c == b':'
            } else {
                c.is_ascii_digit()
            }
        })
        || fraction.is_some_and(|fraction| clock.len() != 8 || fraction.is_empty())
    {
        return Err("expected HH:MM[:SS[.ffffff]]".into());
    }
    let fields = s.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&fields.len()) {
        return Err("expected HH:MM[:SS]".into());
    }
    let hour = number(fields[0])?;
    let minute = number(fields[1])?;
    let (second, micros) = if let Some(text) = fields.get(2) {
        let (second, fraction) = text.split_once('.').unwrap_or((text, ""));
        if fraction.len() > 6 || !fraction.bytes().all(|c| c.is_ascii_digit()) {
            return Err("invalid time fraction".into());
        }
        (
            number(second)?,
            if fraction.is_empty() {
                0
            } else {
                number(&format!("{fraction:0<6}"))?
            },
        )
    } else {
        (0, 0)
    };
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..60).contains(&second) {
        return Err("invalid time".into());
    }
    Ok(Time(
        ((hour * 60 + minute) * 60 + second) * 1_000_000 + micros,
    ))
}
/// Parse a UTC datetime literal.
pub fn datetime(s: &str) -> Result<EngineTime, String> {
    offset_datetime(s).map(|(instant, _)| instant)
}
/// Parse an integral compound duration such as 1h30m.
pub fn duration(mut s: &str) -> Result<EngineDelta, String> {
    let mut result = 0_i64;
    while !s.is_empty() {
        let n = s.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return Err("invalid duration number".into());
        }
        let value = number(&s[..n])?;
        s = &s[n..];
        let n = s.bytes().take_while(u8::is_ascii_alphabetic).count();
        let factor = match &s[..n] {
            "us" => 1,
            "ms" => 1000,
            "s" => 1_000_000,
            "m" => 60_000_000,
            "h" => 3_600_000_000,
            "d" => DAY,
            _ => return Err("unsupported duration unit".into()),
        };
        result = result
            .checked_add(value.checked_mul(factor).ok_or("duration overflow")?)
            .ok_or("duration overflow")?;
        s = &s[n..];
    }
    Ok(EngineDelta::from_micros(result))
}
/// Python-compatible calendar date text.
pub fn date_text(value: Date) -> String {
    let (y, m, d) = components(value);
    format!("{y:04}-{m:02}-{d:02}")
}
/// Python-compatible clock text.
pub fn time_text(value: Time) -> String {
    let seconds = value.0 / 1_000_000;
    let base = format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    );
    if value.0 % 1_000_000 == 0 {
        base
    } else {
        format!("{base}.{:06}", value.0 % 1_000_000)
    }
}
/// Python-compatible UTC instant text without an offset suffix.
pub fn datetime_text(value: EngineTime) -> String {
    format!(
        "{} {}",
        date_text(Date(value.micros().div_euclid(DAY))),
        time_text(Time(value.micros().rem_euclid(DAY)))
    )
}
/// Python-compatible normalized duration text.
pub fn duration_text(value: EngineDelta) -> String {
    let days = value.micros().div_euclid(DAY);
    let clock = time_text(Time(value.micros().rem_euclid(DAY)));
    let clock = clock.strip_prefix('0').unwrap_or(&clock);
    if days == 0 {
        clock.to_owned()
    } else {
        format!(
            "{days} day{}, {clock}",
            if days.abs() == 1 { "" } else { "s" }
        )
    }
}

/// Parse local civil fields without interpreting them as an instant.
pub fn civil_datetime(s: &str) -> Result<CivilDateTime, String> {
    let (day, clock) = s.split_once('T').ok_or("expected date and time")?;
    Ok(CivilDateTime::from_micros(
        date(day)?.0 * DAY + time(clock)?.0,
    ))
}
/// Parse an explicit offset and retain its exact UTC displacement in seconds.
pub fn offset_datetime(s: &str) -> Result<(EngineTime, i32), String> {
    let (civil, offset) = if let Some(civil) = s.strip_suffix('Z') {
        (civil, 0)
    } else {
        let (_, clock) = s.split_once('T').ok_or("expected date and time")?;
        let index = clock
            .find(['+', '-'])
            .ok_or("datetime requires Z or an explicit offset")?;
        let offset = &clock[index..];
        if !matches!(offset.len(), 3 | 6)
            || !offset[1..].bytes().enumerate().all(|(i, c)| {
                if i == 2 {
                    c == b':'
                } else {
                    c.is_ascii_digit()
                }
            })
        {
            return Err("expected offset +HH[:MM] or -HH[:MM]".into());
        }
        let hour = number(&offset[1..3])?;
        let minute = if offset.len() == 6 {
            number(&offset[4..])?
        } else {
            0
        };
        if hour > 23 || minute > 59 {
            return Err("offset outside 23:59".into());
        }
        let seconds = (hour * 3600 + minute * 60) * if offset.starts_with('-') { -1 } else { 1 };
        (
            &s[..s.len() - offset.len()],
            i32::try_from(seconds).map_err(|error| error.to_string())?,
        )
    };
    let micros = civil_datetime(civil)?
        .micros()
        .checked_sub(i64::from(offset) * 1_000_000)
        .ok_or("datetime overflow")?;
    if !(start(1) * DAY..start(10000) * DAY).contains(&micros) {
        return Err("datetime UTC value outside years 0001 through 9999".into());
    }
    Ok((EngineTime::from_micros(micros), offset))
}
