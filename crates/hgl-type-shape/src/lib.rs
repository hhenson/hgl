//! Canonical checked source shapes and invariant nominal applications.
use std::fmt;

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
            self.arguments
                .iter()
                .map(Ty::source_name)
                .collect::<Vec<_>>()
                .join(",")
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
    /// Complete ordinary payload behind one temporal boundary.
    Atomic(Box<Self>),
    /// Integer-keyed temporal map and recursively checked child shape.
    Map(Box<Self>, Box<Self>),
    /// Positional temporal children.
    Tuple(Vec<Self>),
    /// Ordinary publication data retaining its exact structural origin.
    Delta(Box<Self>),
    /// Ordinary list element and optional exact fixed length.
    List(Box<Self>, Option<usize>),
    /// Qualified nominal identity and required ordinary field types.
    Struct(Nominal, Vec<(String, Self)>),
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
    /// Reference designation.
    Ref(Box<Self>),
    /// Set membership.
    Set(Box<Self>),
    /// Contextual nullable expression; not an admitted source annotation.
    Nullable(Box<Self>),
    /// No result.
    Void,
}
impl Ty {
    /// Canonical scalar spelling; constructed types retain their child separately.
    pub fn name(&self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Duration => "duration",
            Self::Date => "date",
            Self::Time => "time",
            Self::DateTime => "datetime",
            Self::Ref(_) => "ref",
            Self::Set(_) => "set",
            Self::Nullable(_) => "contextual nullable",
            Self::Void => "void",
            Self::Struct(..) => "struct",
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
            "void" => Some(Self::Void),
            _ => None,
        }
    }
    /// Split an ordinary list spelling, respecting nested type arguments.
    pub fn list_parts(name: &str) -> Option<(&str, Option<usize>)> {
        let body = name.strip_prefix("list<")?.strip_suffix('>')?;
        let mut depth = 0;
        for (index, ch) in body.char_indices() {
            if matches!(ch, '<' | '(') {
                depth += 1;
            }
            if matches!(ch, '>' | ')') {
                depth -= 1;
            }
            if ch == ',' && depth == 0 {
                let size = &body[index + 1..];
                return Some((
                    &body[..index],
                    if size == "unbounded" {
                        None
                    } else {
                        Some(usize::try_from(size.parse::<i64>().ok()?).ok()?)
                    },
                ));
            }
        }
        Some((body, None))
    }
    /// Canonical checked HGL source form, including complete nominal arguments.
    pub fn source_name(&self) -> String {
        match self {
            Self::Struct(identity, _) => identity.source_name(),
            Self::Delta(origin) => format!("delta<{}>", origin.source_name()),
            Self::Tuple(children) => format!(
                "tuple<{}>",
                children
                    .iter()
                    .map(Self::source_name)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
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
            | Self::Void => self.name().into(),
        }
    }
}
/// Split an outer application without confusing nested argument separators.
pub fn application(name: &str) -> Option<(&str, Vec<&str>)> {
    let (origin, body) = name.split_once('<')?;
    if origin.contains(['(', ')']) {
        return None;
    }
    let body = body.strip_suffix('>')?;
    let mut depth = 0_i32;
    let mut parentheses = 0_i32;
    let mut start = 0;
    let mut arguments = Vec::new();
    for (index, ch) in body.char_indices() {
        match ch {
            '(' => parentheses += 1,
            ')' => parentheses -= 1,
            '<' if parentheses == 0 => depth += 1,
            '>' if parentheses == 0 => depth -= 1,
            ',' if depth == 0 && parentheses == 0 => {
                arguments.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        if depth < 0 || parentheses < 0 {
            return None;
        }
    }
    if depth != 0 || parentheses != 0 {
        return None;
    }
    arguments.push(&body[start..]);
    Some((origin, arguments))
}

/// Recognize the contextual type relationship without consuming nested applications.
pub fn delta_argument(name: &str) -> Option<&str> {
    let (base, arguments) = application(name)?;
    if base == "delta" && arguments.len() == 1 && !arguments[0].is_empty() {
        Some(arguments[0])
    } else {
        None
    }
}
impl Ty {
    /// Whether this exact type belongs to the finite publication profile.
    pub fn publication(&self) -> bool {
        match self {
            Self::Bool
            | Self::I64
            | Self::F64
            | Self::Str
            | Self::Date
            | Self::Time
            | Self::DateTime
            | Self::Duration => true,
            Self::Atomic(payload) => payload.atomic_payload(),
            Self::Set(member) => matches!(**member, Self::Bool | Self::I64),
            Self::List(child, Some(_)) => child.publication(),
            Self::Tuple(children) => children.iter().all(Self::publication),
            Self::Struct(_, fields) => fields.iter().all(|(_, child)| child.publication()),
            Self::Map(key, child) => **key == Self::I64 && child.publication(),
            Self::List(_, None)
            | Self::Delta(_)
            | Self::Ref(_)
            | Self::Nullable(_)
            | Self::Void => false,
        }
    }
    /// Form the exact ordinary publication type, reducing scalar origins.
    pub fn delta(self) -> Result<Self, String> {
        if !self.publication() {
            return Err(format!(
                "delta: unsupported publication shape {}",
                self.source_name()
            ));
        }
        if let Self::Atomic(payload) = self {
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
        if let Self::List(child, _) = self {
            return child.atomic_payload();
        }
        if let Self::Tuple(children) = self {
            return children.iter().all(Self::atomic_payload);
        }
        if let Self::Struct(_, fields) = self {
            return fields.iter().all(|(_, ty)| ty.atomic_payload());
        }
        self.scalar()
    }
    fn scalar(&self) -> bool {
        matches!(
            self,
            Self::Bool
                | Self::I64
                | Self::F64
                | Self::Str
                | Self::Date
                | Self::Time
                | Self::DateTime
                | Self::Duration
        )
    }
}
