//! Check sparse ordinary delta formation without evaluating payload expressions.
use crate::composite_keys::{Key, key};
use crate::ir::{DeltaEntry, Kind, Value};
use hgl_source::{Expr, Literal, Ty};
use std::collections::BTreeSet;

/// An admitted constructor component, in source evaluation order.
#[derive(Debug)]
pub enum Part<'a> {
    /// Constant set addition, possibly requiring cold provider materialization.
    Added(Value),
    /// Constant set member or map key removal.
    Removed(Value),
    /// Exact complete map key and unevaluated child payload.
    Keyed(Value, Ty, &'a Expr),
    /// Field/position, exact derived child type and unevaluated payload.
    Child(i64, Ty, &'a Expr),
}
/// Validate a complete constructor before checking or evaluating payloads.
pub fn constructor<'a>(
    origin: &Ty,
    args: &'a [(Option<String>, Expr)],
    mut fixed: impl FnMut(&Expr, &Ty) -> Result<(Value, Option<Value>), String>,
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
                for value in members(expr, member, &mut fixed)? {
                    let keys = if name == "added" {
                        &mut added
                    } else {
                        &mut removed
                    };
                    remember(&value, member, keys, "set member")?;
                    parts.push(if name == "added" {
                        Part::Added(value.0)
                    } else {
                        Part::Removed(value.0)
                    });
                }
            }
            Ty::List(_, None) if name == "remove" => {
                parts.extend(removals(expr, &mut fixed, &mut removed, &Ty::I64, true)?);
            }
            Ty::Map(member, _) if name == "remove" => {
                parts.extend(removals(expr, &mut fixed, &mut removed, member, false)?);
            }
            Ty::Map(..) | Ty::List(..) | Ty::Tuple(_)
                if matches!(
                    (origin, name),
                    (Ty::Map(..), "upsert") | (Ty::List(..) | Ty::Tuple(_), "items")
                ) =>
            {
                sparse(origin, expr, &mut fixed, &mut added, &mut parts)?;
            }
            Ty::Struct(_, fields, _) => parts.push(field(fields, name, expr)?),
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
            | Ty::Rolling(..)
            | Ty::Ref(_)
            | Ty::Set(_)
            | Ty::Nullable(_)
            | Ty::Atomic(_)
            | Ty::Recursive(_)
            | Ty::Family(_)
            | Ty::Void => return Err(format!("unknown delta argument {name}")),
        }
    }
    disjoint(&added, &removed)?;
    Ok(parts)
}
fn field<'a>(fields: &[(String, Ty)], name: &str, expr: &'a Expr) -> Result<Part<'a>, String> {
    let (index, (_, child)) = fields
        .iter()
        .enumerate()
        .find(|(_, (field, _))| field == name)
        .ok_or_else(|| format!("unknown delta field {name}"))?;
    Ok(Part::Child(
        i64::try_from(index).map_err(|e| e.to_string())?,
        child.clone().delta()?,
        expr,
    ))
}

fn remember(
    value: &(Value, Option<Value>),
    expected: &Ty,
    keys: &mut BTreeSet<Key>,
    what: &str,
) -> Result<(), String> {
    if value.0.ty != *expected {
        return Err(format!(
            "delta {what} type mismatch: requires constant {}",
            expected.source_name()
        ));
    }
    if let Some(value) = &value.1
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
    member: &Ty,
    fixed: &mut impl FnMut(&Expr, &Ty) -> Result<(Value, Option<Value>), String>,
) -> Result<Vec<(Value, Option<Value>)>, String> {
    let Expr::Sequence(values) = expr else {
        return Err("delta membership/removal requires a constant literal list".into());
    };
    values
        .iter()
        .map(|value| {
            fixed(
                value.as_ref().ok_or("delta member cannot be absent")?,
                member,
            )
        })
        .collect()
}
fn sparse<'a>(
    origin: &Ty,
    expr: &'a Expr,
    fixed: &mut impl FnMut(&Expr, &Ty) -> Result<(Value, Option<Value>), String>,
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
        let expected = if let Ty::Map(member, _) = origin {
            member.as_ref()
        } else {
            &Ty::I64
        };
        let position = fixed(position, expected)?;
        if let Ty::Map(member, child) = origin {
            remember(&position, member, supplied, "map key")?;
            parts.push(Part::Keyed(position.0, child.clone().delta()?, payload));
            continue;
        }
        let (
            _,
            Some(Value {
                kind: Kind::Literal(Literal::Int(index)),
                ..
            }),
        ) = position
        else {
            return Err("delta index requires constant i64".into());
        };
        remember(
            &(
                Value::new(Ty::I64, Kind::Literal(Literal::Int(index))),
                Some(Value::new(Ty::I64, Kind::Literal(Literal::Int(index)))),
            ),
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
            Ty::List(child, None) if index >= 0 => child.as_ref(),
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
            | Ty::Rolling(..)
            | Ty::Ref(_)
            | Ty::Set(_)
            | Ty::Nullable(_)
            | Ty::Atomic(_)
            | Ty::Recursive(_)
            | Ty::Family(_)
            | Ty::Void => return Err("delta index out of bounds".into()),
        };
        parts.push(Part::Child(index, child.clone().delta()?, payload));
    }
    Ok(())
}
/// Check payloads and retain every complete key recipe in its written position.
pub fn values(
    parts: Vec<Part<'_>>,
    mut check: impl FnMut(&Ty, &Expr) -> Result<Value, String>,
) -> Result<Vec<DeltaEntry>, String> {
    parts
        .into_iter()
        .map(|part| {
            Ok(match part {
                Part::Added(value) => DeltaEntry::Add(value),
                Part::Removed(value) => DeltaEntry::Remove(value),
                Part::Child(index, ty, expr) => DeltaEntry::Child(index, check(&ty, expr)?),
                Part::Keyed(key, ty, expr) => DeltaEntry::Keyed(key, check(&ty, expr)?),
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

fn nonnegative(value: &(Value, Option<Value>)) -> Result<(), String> {
    if matches!(&value.1,Some(Value {kind:Kind::Literal(Literal::Int(index)),..}) if *index>=0) {
        Ok(())
    } else {
        Err("delta index requires nonnegative constant i64".into())
    }
}

fn removals<'a>(
    expr: &Expr,
    fixed: &mut impl FnMut(&Expr, &Ty) -> Result<(Value, Option<Value>), String>,
    removed: &mut BTreeSet<Key>,
    member: &Ty,
    index: bool,
) -> Result<Vec<Part<'a>>, String> {
    members(expr, member, fixed)?
        .into_iter()
        .map(|value| {
            if index {
                nonnegative(&value)?;
            }
            remember(
                &value,
                member,
                removed,
                if index { "index" } else { "map key" },
            )?;
            Ok(Part::Removed(value.0))
        })
        .collect()
}

/// Check publication access after phase and argument evaluation.
pub fn delta_value(value: Value, facts: &BTreeSet<(String, usize)>) -> Result<Value, String> {
    let Kind::Input(id, false) = value.kind else {
        return Err(
            "delta_value requires a temporal input endpoint; signal is not admitted".into(),
        );
    };
    if !value.ty.publication() {
        return Err("delta_value: unsupported publication shape".into());
    }
    if !["valid", "modified"]
        .iter()
        .all(|q| facts.contains(&(q.to_string(), id)))
    {
        return Err("delta_value requires proof that its endpoint is valid and modified".into());
    }
    Ok(Value::new(
        value.ty.clone().delta()?,
        Kind::Query("delta_value".into(), vec![value]),
    ))
}
