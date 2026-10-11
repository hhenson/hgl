//! Source literal parsing; retained scalar forms belong to the model.
pub use hgl_value_model::literals::*;
fn zone_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 255
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'+' | b'/'))
        || name.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err("invalid timezone name syntax".into());
    }
    Ok(())
}
fn temporal(text: &str) -> Result<ParsedLiteral, String> {
    if let Some((value, name)) = text.split_once('[') {
        let name = name
            .strip_suffix(']')
            .ok_or("unterminated timezone annotation")?;
        zone_name(name)?;
        if value.is_empty() {
            return Ok(ParsedLiteral::Contextual(TemporalLiteral::TimeZone(
                name.into(),
            )));
        }
        if !value.contains('T') {
            return Ok(ParsedLiteral::Contextual(TemporalLiteral::ZonedTime {
                time_micros: hgl_types::calendar::time(value)?.0,
                zone: name.into(),
            }));
        }
        let (instant, offset_seconds) = hgl_types::calendar::offset_datetime(value).map_err(|error| format!("zoned_datetime requires an explicit valid offset; use resolve for civil values: {error}"))?;
        return Ok(ParsedLiteral::Contextual(TemporalLiteral::ZonedDateTime {
            instant_micros: instant.micros(),
            zone: name.into(),
            offset_seconds,
        }));
    }
    let value = if let Some((_, clock)) = text.split_once('T') {
        if clock.ends_with('Z') || clock.contains(['+', '-']) {
            Literal::DateTime(hgl_types::calendar::offset_datetime(text)?.0.micros())
        } else {
            Literal::CivilDateTime(hgl_types::calendar::civil_datetime(text)?.micros())
        }
    } else if text.contains(':') {
        Literal::Time(hgl_types::calendar::time(text)?.0)
    } else {
        Literal::Date(hgl_types::calendar::date(text)?.0)
    };
    Ok(ParsedLiteral::Value(value))
}

/// Parse a numeric or temporal token without consulting a provider.
pub fn numeric(text: &str, negative: bool) -> Result<Option<ParsedLiteral>, String> {
    if let Some(text) = text.strip_prefix('@') {
        if negative {
            return Err("cannot negate a calendar literal".into());
        }
        return temporal(text).map(Some);
    }
    if !text.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return Ok(None);
    }
    if text.contains('.') || text.contains('e') || text.contains('E') {
        let value = text
            .parse::<f64>()
            .map_err(|e| format!("invalid float: {e}"))?;
        if !value.is_finite() {
            return Err("float literal is outside the finite f64 range".into());
        }
        return Ok(Some(ParsedLiteral::Value(Literal::Float(if negative {
            -value
        } else {
            value
        }))));
    }
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits < text.len() {
        let value = hgl_types::calendar::duration(text)?.micros();
        return Ok(Some(ParsedLiteral::Value(Literal::Duration(if negative {
            -value
        } else {
            value
        }))));
    }
    let value = format!("{}{text}", if negative { "-" } else { "" })
        .parse::<i64>()
        .map_err(|e| format!("invalid i64: {e}"))?;
    Ok(Some(ParsedLiteral::Value(Literal::Int(value))))
}
