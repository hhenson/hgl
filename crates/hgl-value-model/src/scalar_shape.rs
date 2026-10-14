//! Builtin scalar metadata shared by cold frontend and backend layout planning.
use crate::Ty;
use hgl_types::ScalarType;
impl Ty {
    /// Exact builtin scalar column kind; nominal and owning whole-value leaves stay separate.
    pub fn scalar_kind(&self) -> Option<ScalarType> {
        Some(match self {
            Self::Bool => ScalarType::Bool,
            Self::I64 => ScalarType::I64,
            Self::F64 => ScalarType::F64,
            Self::Str => ScalarType::Text,
            Self::Bytes => ScalarType::Bytes,
            Self::Date => ScalarType::Date,
            Self::Time => ScalarType::Time,
            Self::DateTime => ScalarType::DateTime,
            Self::Duration => ScalarType::Duration,
            Self::CivilDateTime => ScalarType::CivilDateTime,
            Self::TimeZone => ScalarType::TimeZone,
            Self::ZonedTime => ScalarType::ZonedTime,
            Self::ZonedDateTime => ScalarType::ZonedDateTime,
            Self::Result(..)
            | Self::Error
            | Self::Rolling(..)
            | Self::Family(_)
            | Self::Recursive(_)
            | Self::Enum(_)
            | Self::Native(_)
            | Self::Atomic(_)
            | Self::Map(..)
            | Self::Tuple(_)
            | Self::Delta(_)
            | Self::List(..)
            | Self::Struct(..)
            | Self::Any
            | Self::Ref(_)
            | Self::Set(_)
            | Self::Nullable(_)
            | Self::Void => return None,
        })
    }
    /// Whether an ordinary scalar owns variable-sized payload storage.
    pub fn owning_scalar(&self) -> bool {
        if let Self::Result(ok, err) = self {
            return ok.owning_scalar() || err.owning_scalar();
        }
        matches!(
            self,
            Self::Error
                | Self::Str
                | Self::Bytes
                | Self::Any
                | Self::Native(_)
                | Self::TimeZone
                | Self::ZonedTime
                | Self::ZonedDateTime
        )
    }
}
