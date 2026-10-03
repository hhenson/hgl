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
        }
    }
    /// Parse a concrete admitted type spelling.
    pub fn parse(name: &str) -> Option<Self> {
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
            if ch == '<' {
                depth += 1;
            }
            if ch == '>' {
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
}

impl Ty {
    /// Canonical checked HGL source form, including complete nominal arguments.
    pub fn source_name(&self) -> String {
        match self {
            Self::Struct(identity, _) => identity.source_name(),
            Self::List(element, size) => match size {
                Some(size) => format!("list<{},{}>", element.source_name(), size),
                None => format!("list<{}>", element.source_name()),
            },
            Self::Ref(child) | Self::Set(child) | Self::Nullable(child) => {
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
    let body = body.strip_suffix('>')?;
    let mut depth = 0_i32;
    let mut start = 0;
    let mut arguments = Vec::new();
    for (index, ch) in body.char_indices() {
        match ch {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                arguments.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
    }
    if depth != 0 {
        return None;
    }
    arguments.push(&body[start..]);
    Some((origin, arguments))
}
