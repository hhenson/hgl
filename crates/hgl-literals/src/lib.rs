//! Scalar source values and unresolved execution-context literal recipes.
use hgl_type_shape::{EnumType, Ty};
use hgl_types::time_values::{ZoneId, ZonedDateTime, ZonedTime};
#[derive(Debug, Clone, PartialEq)]
/// A fixed scalar value in HGL source.
pub enum Literal {
    /// Declared enum identity and assigned member number.
    Enum(EnumType, i64),
    /// Signed integer value.
    Int(i64),
    /// Floating value.
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// UTF-8 text.
    Str(String),
    /// Microsecond interval.
    Duration(i64),
    /// Calendar date.
    Date(i64),
    /// Time of day.
    Time(i64),
    /// UTC instant.
    DateTime(i64),
    /// Local civil microseconds, with no timeline interpretation.
    CivilDateTime(i64),
    /// Constructed exact-name zone value.
    TimeZone(ZoneId),
    /// Constructed wall-clock time and exact zone.
    ZonedTime(ZonedTime),
    /// Constructed instant, exact zone and resolved offset.
    ZonedDateTime(ZonedDateTime),
}
impl Literal {
    /// The literal scalar type.
    pub fn ty(&self) -> Ty {
        match self {
            Self::Enum(ty, _) => Ty::Enum(ty.clone()),
            Self::Int(_) => Ty::I64,
            Self::Float(_) => Ty::F64,
            Self::Bool(_) => Ty::Bool,
            Self::Str(_) => Ty::Str,
            Self::Duration(_) => Ty::Duration,
            Self::Date(_) => Ty::Date,
            Self::Time(_) => Ty::Time,
            Self::DateTime(_) => Ty::DateTime,
            Self::CivilDateTime(_) => Ty::CivilDateTime,
            Self::TimeZone(_) => Ty::TimeZone,
            Self::ZonedTime(_) => Ty::ZonedTime,
            Self::ZonedDateTime(_) => Ty::ZonedDateTime,
        }
    }
}

/// Provider-dependent source literal, not yet an ordinary value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalLiteral {
    /// Exact zone spelling, syntactically checked only.
    TimeZone(String),
    /// Wall-clock time requiring exact provider catalog membership.
    ZonedTime {
        /// Microseconds after midnight, without a date or offset.
        time_micros: i64,
        /// Exact written name, never canonicalized.
        zone: String,
    },
    /// Explicit-offset proposal requiring strict provider agreement.
    ZonedDateTime {
        /// The instant determined by the written offset.
        instant_micros: i64,
        /// Exact written name, never canonicalized.
        zone: String,
        /// Written UTC offset in seconds.
        offset_seconds: i32,
    },
}
impl TemporalLiteral {
    /// The source type, independent of provider validity.
    pub fn ty(&self) -> Ty {
        match self {
            Self::TimeZone(_) => Ty::TimeZone,
            Self::ZonedTime { .. } => Ty::ZonedTime,
            Self::ZonedDateTime { .. } => Ty::ZonedDateTime,
        }
    }
}
/// Parsing distinguishes closed values from execution-context recipes.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLiteral {
    /// Provider-independent ordinary literal.
    Value(Literal),
    /// Requires the execution context before it becomes a value.
    Contextual(TemporalLiteral),
}
impl ParsedLiteral {
    /// The scalar type before any contextual materialization.
    pub fn ty(&self) -> Ty {
        match self {
            Self::Value(value) => value.ty(),
            Self::Contextual(value) => value.ty(),
        }
    }
}
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
