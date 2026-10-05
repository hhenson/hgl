//! Static source-to-native scalar spellings.
use hgl_source::Ty;
/// Emit the checked rust type form.
pub fn rust_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::I64 => "i64",
        Ty::Duration => "hgl_types::EngineDelta",
        Ty::Date => "hgl_types::Date",
        Ty::Time => "hgl_types::Time",
        Ty::DateTime => "hgl_types::EngineTime",
        Ty::CivilDateTime => "hgl_types::CivilDateTime",
        Ty::TimeZone => "hgl_types::ZoneId",
        Ty::ZonedDateTime => "hgl_types::ZonedDateTime",
        Ty::Bool => "bool",
        Ty::F64 => "f64",
        Ty::Str => "String",
        Ty::Void => "()",
        Ty::Atomic(_)
        | Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Delta(_)
        | Ty::Struct(..)
        | Ty::List(..)
        | Ty::Nullable(_) => {
            unreachable!("ordinary aggregate and nullable locals use inferred Rust types")
        }
        Ty::Ref(_) => "hgl_store::Reference",
        Ty::Set(_) => "hgl_store::InputId",
    }
}
/// Emit the checked scalar type form.
pub fn scalar_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::Bool => "Bool",
        Ty::F64 => "F64",
        Ty::I64 => "I64",
        Ty::Str => "Text",
        Ty::Duration => "Duration",
        Ty::Date => "Date",
        Ty::Time => "Time",
        Ty::DateTime => "DateTime",
        Ty::CivilDateTime => "CivilDateTime",
        Ty::TimeZone => "TimeZone",
        Ty::ZonedDateTime => "ZonedDateTime",
        Ty::Atomic(_)
        | Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Delta(_)
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Struct(..)
        | Ty::List(..)
        | Ty::Void => {
            unreachable!("checked endpoint type")
        }
    }
}
