//! Canonical checked source shapes and invariant nominal applications.
pub use hgl_types::window_types::{Window, WindowKind};
use std::fmt;
/// Fixed declared membership for an abstract atomic family.
pub type FamilyType = crate::nominal_batch::Family<Nominal, Ty>;
/// Finite recursive source batch with invariant concrete nominal identity.
pub type RecursiveType = crate::nominal_batch::Batch<Nominal, Ty>;
/// Concrete recursive member schema retained at cold checked boundaries.
pub type NominalDefinition = crate::nominal_batch::Definition<Nominal, Ty>;

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
        format!(
            "{}<{}>",
            self.origin,
            type_names(self.arguments.iter().map(Ty::source_name))
        )
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
    /// One complete ordinary success/error outcome; intrinsically atomic temporally.
    Result(Box<Self>, Box<Self>),
    /// Canonical ordinary diagnostic; intrinsically atomic temporally.
    Error,

    /// Inherently temporal arrival window with exact resolved bounds.
    Rolling(Box<Self>, Window),
    /// Closed declared concrete membership of an abstract atomic family.
    Family(FamilyType),
    /// Complete finite recursive batch, or nominal edge within such a batch.
    Recursive(RecursiveType),
    /// A declared nominal enum scalar.
    Enum(EnumType),
    /// Descriptor-defined ordinary nominal scalar with declared capabilities.
    Native(crate::native_contracts::Scalar),
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
    /// Uninterpreted octets.
    Bytes,
    /// Independently owned typed contents or nil.
    Any,
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
            Self::Result(..) => "result",
            Self::Error => "error",
            Self::Rolling(..) => "rolling",
            Self::Enum(_) => "enum",
            Self::Native(_) => "native",
            Self::I64 => "i64",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Bytes => "bytes",
            Self::Any => "any",
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
    /// Canonical checked HGL source form, including complete nominal arguments.
    pub fn source_name(&self) -> String {
        match self {
            Self::Result(ok, err) => format!("result<{},{}>", ok.source_name(), err.source_name()),
            Self::Error => "error".into(),
            Self::Rolling(child, window) => {
                format!("rolling<{},{}>", child.source_name(), window.source_name())
            }
            Self::Enum(ty) => ty.origin.clone(),
            Self::Native(ty) => ty.identity.clone(),
            Self::Struct(identity, _, _) => identity.source_name(),
            Self::Recursive(batch) => batch.identity().source_name(),
            Self::Family(family) => family.identity().source_name(),
            Self::Tuple(children) => format!(
                "tuple<{}>",
                type_names(children.iter().map(Ty::source_name))
            ),
            Self::Map(key, child) => format!("map<{},{}>", key.source_name(), child.source_name()),
            Self::List(element, size) => match size {
                Some(size) => format!("list<{},{}>", element.source_name(), size),
                None => format!("list<{}>", element.source_name()),
            },
            Self::Delta(origin) => format!("delta<{}>", origin.source_name()),
            Self::Atomic(child) | Self::Ref(child) | Self::Set(child) | Self::Nullable(child) => {
                format!("{}<{}>", self.name(), child.source_name())
            }
            Self::I64
            | Self::F64
            | Self::Bool
            | Self::Str
            | Self::Bytes
            | Self::Any
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
        if let Self::Result(ok, err) = self {
            return ok.atomic_payload() && err.atomic_payload();
        }
        if let Self::Atomic(payload) | Self::Rolling(payload, _) = self {
            return payload.atomic_payload();
        }
        if let Self::Struct(_, _, optional) = self
            && !optional.is_empty()
        {
            return false;
        }
        self.components(Self::publication)
    }
    /// Form the exact ordinary publication type, reducing scalar origins.
    pub fn delta(self) -> Result<Self, crate::Issue> {
        if !self.publication() {
            return Err(crate::Issue::delta_shape(&self.source_name()));
        }
        if let Self::Atomic(payload) | Self::Rolling(payload, _) = self {
            return Ok(*payload);
        }
        if self.scalar() {
            return Ok(self);
        }
        Ok(Self::Delta(Box::new(self)))
    }
    /// Normalize admitted non-composite spellings before type comparison.
    #[must_use]
    pub fn atomic(self) -> Self {
        if self.scalar() {
            return self;
        }
        Self::Atomic(Box::new(self))
    }
    /// Whether an ordinary value belongs to the finite complete-payload profile.
    pub fn atomic_payload(&self) -> bool {
        if let Self::Result(ok, err) = self {
            return ok.atomic_payload() && err.atomic_payload();
        }
        matches!(self, Self::Error | Self::Recursive(_) | Self::Family(_))
            || self.components(Self::atomic_payload)
    }
    /// Exact finite complete keys exclude recursive, family and collection components.
    pub fn collection_key(&self) -> bool {
        if let Self::Native(scalar) = self {
            return ["equality", "hash"]
                .iter()
                .all(|cap| scalar.capabilities.contains(*cap));
        }
        !matches!(
            self,
            Self::Result(..) | Self::Error | Self::Set(_) | Self::Map(..) | Self::List(..)
        ) && self.components(Self::collection_key)
    }
    fn components(&self, check: fn(&Self) -> bool) -> bool {
        if let Self::Set(key) = self {
            return key.collection_key();
        }
        if let Self::Map(key, child) = self {
            return key.collection_key() && check(child);
        }
        if let Self::List(child, _) = self {
            return check(child);
        }
        if let Self::Tuple(children) = self {
            return children.iter().all(check);
        }
        if let Self::Struct(_, fields, _) = self {
            return fields.iter().all(|(_, ty)| check(ty));
        }
        self.scalar()
    }
    /// Whether this is an ordinary scalar leaf, including nominal enums and boxes.
    pub fn scalar(&self) -> bool {
        matches!(
            self,
            Self::Result(..) | Self::Error | Self::Enum(_) | Self::Native(_) | Self::Any
        ) || self.scalar_kind().is_some()
    }
}

fn type_names(names: impl Iterator<Item = String>) -> String {
    names.collect::<Vec<_>>().join(",")
}
