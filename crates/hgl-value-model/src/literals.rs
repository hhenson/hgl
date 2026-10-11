//! Scalar source values and unresolved execution-context literal recipes.
use crate::type_shape::{EnumType, Ty};
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
    /// Constructed octets, without a source literal token.
    Bytes(Vec<u8>),
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
            Self::Bytes(_) => Ty::Bytes,
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
