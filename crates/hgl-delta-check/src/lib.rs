//! Check sparse ordinary delta formation without evaluating payload expressions.
use hgl_rust_ir::{DeltaEntry, Kind, Value};
use hgl_scalar_keys::{Key, key};
use hgl_source::{Expr, Literal, ParsedLiteral, Ty};
use std::collections::BTreeSet;

/// An admitted constructor component, in source evaluation order.
#[derive(Debug)]
pub enum Part<'a> {
    /// Constant set addition, possibly requiring cold provider materialization.
    Added(ParsedLiteral),
    /// Constant set member or map key removal.
    Removed(ParsedLiteral),
    /// Exact scalar map key and unevaluated child payload.
    Keyed(ParsedLiteral, Ty, &'a Expr),
    /// Field/position, exact derived child type and unevaluated payload.
    Child(i64, Ty, &'a Expr),
}
/// Validate a complete constructor before checking or evaluating payloads.
pub fn constructor<'a>(
    origin: &Ty,
    args: &'a [(Option<String>, Expr)],
    mut fixed: impl FnMut(&Expr) -> Result<ParsedLiteral, String>,
) -> Result<Vec<Part<'a>>, String> {
    structural_origin(origin)?;
    let mut names = BTreeSet::new();
    let mut added = BTreeSet::new();
    let mut removed = BTreeSet::new();
    let mut parts = Vec::new();
    for (name, expr) in args {
        let name = name
            .as_deref()
            .ok_or("delta constructor arguments must be named")?;
        if !names.insert(name) {
            return Err(format!("duplicate delta argument {name}"));
        }
        match origin {
            Ty::Set(member) if matches!(name, "added" | "removed") => {
                for value in members(expr, &mut fixed)? {
                    let keys = if name == "added" {
                        &mut added
                    } else {
                        &mut removed
                    };
                    remember(&value, member, keys, "set member")?;
                    parts.push(if name == "added" {
                        Part::Added(value)
                    } else {
                        Part::Removed(value)
                    });
                }
            }
            Ty::Map(member, _) if name == "remove" => {
                for value in members(expr, &mut fixed)? {
                    remember(&value, member, &mut removed, "map key")?;
                    parts.push(Part::Removed(value));
                }
            }
            Ty::Map(_, _) if name == "upsert" => {
                sparse(origin, expr, &mut fixed, &mut added, &mut parts)?;
            }
            Ty::List(..) | Ty::Tuple(_) if name == "items" => {
                sparse(origin, expr, &mut fixed, &mut added, &mut parts)?;
            }
            Ty::Struct(_, fields) => {
                let (index, (_, child)) = fields
                    .iter()
                    .enumerate()
                    .find(|(_, (field, _))| field == name)
                    .ok_or_else(|| format!("unknown delta field {name}"))?;
                parts.push(Part::Child(
                    i64::try_from(index).map_err(|e| e.to_string())?,
                    child.clone().delta()?,
                    expr,
                ));
            }
            Ty::Map(..)
            | Ty::Tuple(_)
            | Ty::Delta(_)
            | Ty::List(..)
            | Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Str
            | Ty::CivilDateTime
            | Ty::TimeZone
            | Ty::Enum(_)
            | Ty::ZonedTime
            | Ty::ZonedDateTime
            | Ty::Duration
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::Ref(_)
            | Ty::Set(_)
            | Ty::Nullable(_)
            | Ty::Atomic(_)
            | Ty::Void => return Err(format!("unknown delta argument {name}")),
        }
    }
    disjoint(&added, &removed)?;
    Ok(parts)
}
fn remember(
    value: &ParsedLiteral,
    expected: &Ty,
    keys: &mut BTreeSet<Key>,
    what: &str,
) -> Result<(), String> {
    if value.ty() != *expected {
        return Err(format!(
            "delta {what} type mismatch: requires constant {}",
            expected.source_name()
        ));
    }
    if let ParsedLiteral::Value(value) = value
        && !keys.insert(key(value)?)
    {
        return Err(format!("duplicate delta {what}"));
    }
    Ok(())
}
fn disjoint(added: &BTreeSet<Key>, removed: &BTreeSet<Key>) -> Result<(), String> {
    if added.intersection(removed).next().is_some() {
        return Err("delta additions/upserts overlap removals".into());
    }
    Ok(())
}
fn members(
    expr: &Expr,
    fixed: &mut impl FnMut(&Expr) -> Result<ParsedLiteral, String>,
) -> Result<Vec<ParsedLiteral>, String> {
    let Expr::Sequence(values) = expr else {
        return Err("delta membership/removal requires a constant literal list".into());
    };
    values
        .iter()
        .map(|value| fixed(value.as_ref().ok_or("delta member cannot be absent")?))
        .collect()
}
fn sparse<'a>(
    origin: &Ty,
    expr: &'a Expr,
    fixed: &mut impl FnMut(&Expr) -> Result<ParsedLiteral, String>,
    supplied: &mut BTreeSet<Key>,
    parts: &mut Vec<Part<'a>>,
) -> Result<(), String> {
    if matches!(expr, Expr::Sequence(values) if values.is_empty()) {
        return Ok(());
    }
    let Expr::Sparse(entries) = expr else {
        return Err("delta child entries require sparse key:payload syntax".into());
    };
    for (position, payload) in entries {
        let position = fixed(position)?;
        if let Ty::Map(member, child) = origin {
            remember(&position, member, supplied, "map key")?;
            parts.push(Part::Keyed(position, child.clone().delta()?, payload));
            continue;
        }
        let ParsedLiteral::Value(Literal::Int(index)) = position else {
            return Err("delta index requires constant i64".into());
        };
        remember(
            &ParsedLiteral::Value(Literal::Int(index)),
            &Ty::I64,
            supplied,
            "index",
        )?;
        let child = match origin {
            Ty::List(child, Some(size))
                if usize::try_from(index).is_ok_and(|index| index < *size) =>
            {
                child.as_ref()
            }
            Ty::Tuple(children) => children
                .get(usize::try_from(index).map_err(|_range| "delta index out of bounds")?)
                .ok_or("delta index out of bounds")?,
            Ty::Map(..)
            | Ty::Delta(_)
            | Ty::List(..)
            | Ty::Struct(..)
            | Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Str
            | Ty::CivilDateTime
            | Ty::TimeZone
            | Ty::Enum(_)
            | Ty::ZonedTime
            | Ty::ZonedDateTime
            | Ty::Duration
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::Ref(_)
            | Ty::Set(_)
            | Ty::Nullable(_)
            | Ty::Atomic(_)
            | Ty::Void => return Err("delta index out of bounds".into()),
        };
        parts.push(Part::Child(index, child.clone().delta()?, payload));
    }
    Ok(())
}
fn scalar(value: ParsedLiteral) -> Value {
    let ty = value.ty();
    Value::new(
        ty,
        match value {
            ParsedLiteral::Value(value) => Kind::Literal(value),
            ParsedLiteral::Contextual(value) => Kind::TemporalLiteral(value),
        },
    )
}
/// Check payloads and retain every scalar key recipe in its written position.
pub fn values(
    parts: Vec<Part<'_>>,
    mut check: impl FnMut(&Ty, &Expr) -> Result<Value, String>,
) -> Result<Vec<DeltaEntry>, String> {
    parts
        .into_iter()
        .map(|part| {
            Ok(match part {
                Part::Added(value) => DeltaEntry::Add(scalar(value)),
                Part::Removed(value) => DeltaEntry::Remove(scalar(value)),
                Part::Child(index, ty, expr) => DeltaEntry::Child(index, check(&ty, expr)?),
                Part::Keyed(key, ty, expr) => DeltaEntry::Keyed(scalar(key), check(&ty, expr)?),
            })
        })
        .collect()
}
/// Validate exact duplicate and overlap identities after cold materialization.
pub fn materialized(parts: &[DeltaEntry]) -> Result<(), String> {
    let mut added = BTreeSet::new();
    let mut removed = BTreeSet::new();
    for part in parts {
        let (value, keys) = match part {
            DeltaEntry::Add(value) | DeltaEntry::Keyed(value, _) => (value, &mut added),
            DeltaEntry::Remove(value) => (value, &mut removed),
            DeltaEntry::Child(..) => continue,
        };
        let Kind::Literal(value) = &value.kind else {
            return Err("delta key requires a materialized constant scalar".into());
        };
        if !keys.insert(key(value)?) {
            return Err("duplicate delta key/member".into());
        }
    }
    disjoint(&added, &removed)
}

fn structural_origin(origin: &Ty) -> Result<(), String> {
    if !origin.publication()
        || !matches!(
            origin,
            Ty::Set(_) | Ty::Map(..) | Ty::List(..) | Ty::Tuple(_) | Ty::Struct(..)
        )
    {
        return Err("delta constructor requires a supported structural publication shape".into());
    }
    Ok(())
}
