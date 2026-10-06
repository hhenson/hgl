//! Canonical checked source shapes and invariant nominal applications.
pub use hgl_type_syntax::{application, delta_argument};
pub use hgl_window_types::{Window, WindowKind};
use std::fmt;
/// Fixed declared membership for an abstract atomic family.
pub type FamilyType = hgl_nominal_batch::Family<Nominal, Ty>;
/// Finite recursive source batch with invariant concrete nominal identity.
pub type RecursiveType = hgl_nominal_batch::Batch<Nominal, Ty>;
/// Concrete recursive member schema retained at cold checked boundaries.
pub type NominalDefinition = hgl_nominal_batch::Definition<Nominal, Ty>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// An exact nominal enum declaration with its assigned members.
pub struct EnumType {
    /// Canonical module-qualified declaration identity.
    pub origin: String,
    /// Declaration-ordered names and signed assigned numbers.
    pub members: Vec<(String, i64)>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// A declaring origin and its complete invariant source arguments.
pub struct Nominal {
    /// Module-qualified declaration identity.
    pub origin: String,
    /// Checked source argument shapes, including unused parameters.
    pub arguments: Vec<Ty>,
}
impl Nominal {
    /// Canonical specialization spelling, independent of import aliases.
    pub fn source_name(&self) -> String {
        if self.arguments.is_empty() {
            return self.origin.clone();
        }
        format!("{}<{}>", self.origin, type_names(&self.arguments))
    }
}
impl From<String> for Nominal {
    fn from(origin: String) -> Self {
        Self {
            origin,
            arguments: Vec::new(),
        }
    }
}
impl From<&str> for Nominal {
    fn from(origin: &str) -> Self {
        origin.to_owned().into()
    }
}
impl fmt::Display for Nominal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source_name())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Types admitted by the executable source compiler.
pub enum Ty {
    /// Inherently temporal arrival window with exact resolved bounds.
    Rolling(Box<Self>, Window),
    /// Closed declared concrete membership of an abstract atomic family.
    Family(FamilyType),
    /// Complete finite recursive batch, or nominal edge within such a batch.
    Recursive(RecursiveType),
    /// A declared nominal enum scalar.
    Enum(EnumType),
    /// Complete ordinary payload behind one temporal boundary.
    Atomic(Box<Self>),
    /// Scalar-keyed temporal map and recursively checked child shape.
    Map(Box<Self>, Box<Self>),
    /// Positional temporal children.
    Tuple(Vec<Self>),
    /// Ordinary publication data retaining its exact structural origin.
    Delta(Box<Self>),
    /// Ordinary list element and optional exact fixed length.
    List(Box<Self>, Option<usize>),
    /// Qualified identity, declared field types, and optional field positions.
    Struct(Nominal, Vec<(String, Self)>, Vec<usize>),
    /// Signed integer.
    I64,
    /// Binary floating point.
    F64,
    /// Boolean.
    Bool,
    /// UTF-8 text.
    Str,
    /// Microsecond interval.
    Duration,
    /// Calendar date.
    Date,
    /// Time of day.
    Time,
    /// UTC instant.
    DateTime,
    /// Civil wall-clock fields without a zone.
    CivilDateTime,
    /// Exact named timezone identity.
    TimeZone,
    /// Wall-clock time and exact zone, without a date or offset.
    ZonedTime,
    /// Instant, zone and resolved offset.
    ZonedDateTime,
    /// Reference designation.
    Ref(Box<Self>),
    /// Set membership.
    Set(Box<Self>),
    /// Contextual nullable expression; not an admitted source annotation.
    Nullable(Box<Self>),
    /// No result.
    Void,
}
/// Borrowed concrete field schema at a checked ordinary root.
pub type StructFields<'a> = (&'a Nominal, &'a [(String, Ty)], &'a [usize]);
impl Ty {
    /// Resolve a concrete root schema; internal edges require their enclosing batch.
    pub fn structure(&self) -> Result<StructFields<'_>, String> {
        if let Self::Struct(id, fields, optional) = self {
            return Ok((id, fields, optional));
        }
        if let Self::Recursive(batch) = self {
            let schema = batch.definition(batch.identity())?;
            return Ok((schema.identity(), schema.fields(), schema.optional()));
        }
        Err("ordinary struct required".into())
    }
    /// Canonical scalar spelling; constructed types retain their child separately.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rolling(..) => "rolling",
            Self::Enum(_) => "enum",
            Self::I64 => "i64",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Duration => "duration",
            Self::Date => "date",
            Self::Time => "time",
            Self::DateTime => "datetime",
            Self::CivilDateTime => "civil_datetime",
            Self::TimeZone => "timezone",
            Self::ZonedTime => "zoned_time",
            Self::ZonedDateTime => "zoned_datetime",
            Self::Ref(_) => "ref",
            Self::Set(_) => "set",
            Self::Nullable(_) => "contextual nullable",
            Self::Void => "void",
            Self::Struct(..) | Self::Recursive(_) | Self::Family(_) => "struct",
            Self::List(..) => "list",
            Self::Map(..) => "map",
            Self::Tuple(..) => "tuple",
            Self::Delta(..) => "delta",
            Self::Atomic(_) => "atomic",
        }
    }
    /// Parse a concrete admitted type spelling.
    pub fn parse(name: &str) -> Option<Self> {
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
        if let Some(child) = name.strip_prefix("ref<").and_then(|s| s.strip_suffix('>')) {
            return Some(Self::Ref(Box::new(Self::parse(child)?)));
        }
        if let Some(child) = name.strip_prefix("set<").and_then(|s| s.strip_suffix('>')) {
            return Some(Self::Set(Box::new(Self::parse(child)?)));
        }
        if let Some((element, size)) = Self::list_parts(name) {
            return Some(Self::List(Box::new(Self::parse(element)?), size));
        }
        match name {
            "i64" => Some(Self::I64),
            "f64" => Some(Self::F64),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
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
    pub fn list_parts(name: &str) -> Option<(&str, Option<usize>)> {
        hgl_type_syntax::list_parts(name)
    }
    /// Canonical checked HGL source form, including complete nominal arguments.
    pub fn source_name(&self) -> String {
        match self {
            Self::Rolling(child, window) => {
                format!("rolling<{},{}>", child.source_name(), window.source_name())
            }
            Self::Enum(ty) => ty.origin.clone(),
            Self::Struct(identity, _, _) => identity.source_name(),
            Self::Recursive(batch) => batch.identity().source_name(),
            Self::Family(family) => family.identity().source_name(),
            Self::Delta(origin) => format!("delta<{}>", origin.source_name()),
            Self::Tuple(children) => format!("tuple<{}>", type_names(children)),
            Self::Map(key, child) => format!("map<{},{}>", key.source_name(), child.source_name()),
            Self::List(element, size) => match size {
                Some(size) => format!("list<{},{}>", element.source_name(), size),
                None => format!("list<{}>", element.source_name()),
            },
            Self::Atomic(child) | Self::Ref(child) | Self::Set(child) | Self::Nullable(child) => {
                format!("{}<{}>", self.name(), child.source_name())
            }
            Self::I64
            | Self::F64
            | Self::Bool
            | Self::Str
            | Self::Duration
            | Self::Date
            | Self::Time
            | Self::DateTime
            | Self::CivilDateTime
            | Self::TimeZone
            | Self::ZonedTime
            | Self::ZonedDateTime
            | Self::Void => self.name().into(),
        }
    }
}
impl Ty {
    /// Whether this exact type belongs to the finite publication profile.
    pub fn publication(&self) -> bool {
        if let Self::Atomic(payload) | Self::Rolling(payload, _) = self {
            return payload.atomic_payload();
        }
        if let Self::Set(member) = self {
            return member.collection_key();
        }
        if let Self::List(child, _) = self {
            return child.publication();
        }
        if let Self::Map(key, child) = self {
            return key.collection_key() && child.publication();
        }
        if let Self::Struct(_, _, optional) = self
            && !optional.is_empty()
        {
            return false;
        }
        self.components(Self::publication)
    }
    /// Form the exact ordinary publication type, reducing scalar origins.
    pub fn delta(self) -> Result<Self, String> {
        if !self.publication() {
            let name = self.source_name();
            return Err(format!("delta: unsupported publication shape {name}"));
        }
        if let Self::Atomic(payload) | Self::Rolling(payload, _) = self {
            return Ok(*payload);
        }
        Ok(if self.scalar() {
            self
        } else {
            Self::Delta(Box::new(self))
        })
    }
    /// Normalize admitted non-composite spellings before type comparison.
    #[must_use]
    pub fn atomic(self) -> Self {
        if self.scalar() {
            self
        } else {
            Self::Atomic(Box::new(self))
        }
    }
    /// Whether an ordinary value belongs to the finite complete-payload profile.
    pub fn atomic_payload(&self) -> bool {
        if let Self::Set(key) = self {
            return key.collection_key();
        }
        if let Self::Map(key, child) = self {
            return key.collection_key() && child.atomic_payload();
        }
        if let Self::List(child, _) = self {
            child.atomic_payload()
        } else {
            matches!(self, Self::Recursive(_) | Self::Family(_))
                || self.components(Self::atomic_payload)
        }
    }
    /// Exact finite complete keys exclude recursive, family and collection components.
    pub fn collection_key(&self) -> bool {
        self.components(Self::collection_key)
    }
    fn components(&self, check: fn(&Self) -> bool) -> bool {
        if let Self::Tuple(children) = self {
            return children.iter().all(check);
        }
        if let Self::Struct(_, fields, _) = self {
            return fields.iter().all(|(_, ty)| check(ty));
        }
        self.scalar()
    }
    fn scalar(&self) -> bool {
        matches!(
            self,
            Self::Enum(_)
                | Self::Bool
                | Self::I64
                | Self::F64
                | Self::Str
                | Self::Date
                | Self::Time
                | Self::DateTime
                | Self::CivilDateTime
                | Self::TimeZone
                | Self::ZonedTime
                | Self::ZonedDateTime
                | Self::Duration
        )
    }
}

fn type_names(types: &[Ty]) -> String {
    types
        .iter()
        .map(Ty::source_name)
        .collect::<Vec<_>>()
        .join(",")
}
