//! Source spelling conversion above the syntax-neutral model.
pub use crate::type_syntax::{application, delta_argument};
pub use hgl_value_model::type_shape::*;
/// Parse source type spellings into model shapes without resolving declarations.
pub trait TypeSpelling: Sized {
    /// Parse a concrete built-in source spelling.
    fn parse(name: &str) -> Option<Self>;
    /// Decompose a list source spelling.
    fn list_parts(name: &str) -> Option<(&str, Option<usize>)>;
}
impl TypeSpelling for Ty {
    /// Parse a concrete admitted type spelling.
    fn parse(name: &str) -> Option<Self> {
        if let Some(origin) = delta_argument(name) {
            return Self::parse(origin)?.delta().ok();
        }
        if let Some((base, arguments)) = application(name) {
            if base == "rolling" && (2..=3).contains(&arguments.len()) {
                return Some(Self::Rolling(
                    Box::new(Self::parse(arguments[0])?),
                    Window::parse(arguments[1], arguments.get(2).copied())?,
                ));
            }
            if base == "atomic" && arguments.len() == 1 {
                return Some(Self::parse(arguments[0])?.atomic());
            }
            if base == "ref" && arguments.len() == 1 {
                return Some(Self::Ref(Box::new(Self::parse(arguments[0])?)));
            }
            if base == "set" && arguments.len() == 1 {
                return Some(Self::Set(Box::new(Self::parse(arguments[0])?)));
            }
            if base == "tuple" {
                return Some(Self::Tuple(
                    arguments
                        .into_iter()
                        .map(Self::parse)
                        .collect::<Option<_>>()?,
                ));
            }
            if base == "map" && arguments.len() == 2 {
                return Some(Self::Map(
                    Box::new(Self::parse(arguments[0])?),
                    Box::new(Self::parse(arguments[1])?),
                ));
            }
        }
        if let Some((element, size)) = Self::list_parts(name) {
            return Some(Self::List(Box::new(Self::parse(element)?), size));
        }
        match name {
            "i64" => Some(Self::I64),
            "f64" => Some(Self::F64),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
            "bytes" => Some(Self::Bytes),
            "any" => Some(Self::Any),
            "duration" => Some(Self::Duration),
            "date" => Some(Self::Date),
            "time" => Some(Self::Time),
            "datetime" => Some(Self::DateTime),
            "civil_datetime" => Some(Self::CivilDateTime),
            "timezone" => Some(Self::TimeZone),
            "zoned_time" => Some(Self::ZonedTime),
            "zoned_datetime" => Some(Self::ZonedDateTime),
            "void" => Some(Self::Void),
            _ => None,
        }
    }
    /// Split an ordinary list spelling, respecting nested type arguments.
    fn list_parts(name: &str) -> Option<(&str, Option<usize>)> {
        crate::type_syntax::list_parts(name)
    }
}
