//! Exact scalar identities for cold collection checking, never runtime lookup.
use hgl_source::{Literal, Ty};

/// A comparison key preserving ordinary type and complete scalar identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key(Ty, Identity);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Identity {
    Integer(i64),
    Float(u64),
    Text(String),
    Bytes(Vec<u8>),
    Zoned(i64, String, i32),
}
/// Derive a cold comparison key; signed zeros coincide and NaN is unsupported.
pub fn key(value: &Literal) -> Result<Key, String> {
    let identity = match value {
        Literal::Int(n)
        | Literal::Duration(n)
        | Literal::Date(n)
        | Literal::Time(n)
        | Literal::DateTime(n)
        | Literal::CivilDateTime(n)
        | Literal::Enum(_, n) => Identity::Integer(*n),
        Literal::Bool(value) => Identity::Integer(i64::from(*value)),
        Literal::Float(value) => {
            if value.is_nan() {
                return Err("NaN collection keys and members are unsupported".into());
            }
            Identity::Float(if *value == 0.0 { 0 } else { value.to_bits() })
        }
        Literal::Str(value) => Identity::Text(value.clone()),
        Literal::Bytes(value) => Identity::Bytes(value.clone()),
        Literal::TimeZone(value) => Identity::Text(value.as_str().into()),
        Literal::ZonedTime(value) => {
            Identity::Zoned(value.time().0, value.zone().as_str().into(), 0)
        }
        Literal::ZonedDateTime(value) => Identity::Zoned(
            value.instant().micros(),
            value.zone().as_str().into(),
            value.offset_seconds(),
        ),
    };
    Ok(Key(value.ty(), identity))
}
