//! Check sparse ordinary delta formation without evaluating payload expressions.
use hgl_source::{Expr, Literal, Ty};
use std::collections::BTreeSet;

/// An admitted constructor component, in source evaluation order.
#[derive(Debug)]
pub enum Part<'a> {
    /// Constant set addition.
    Added(Literal),
    /// Constant set member or map key removal.
    Removed(Literal),
    /// Field/position/key, exact derived child type and unevaluated payload.
    Child(i64, Ty, &'a Expr),
}
/// Validate a complete constructor before checking or evaluating payloads.
pub fn constructor<'a>(
    origin: &Ty,
    args: &'a [(Option<String>, Expr)],
    mut fixed: impl FnMut(&Expr) -> Result<Literal, String>,
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
                set_members(
                    member,
                    name,
                    members(expr, &mut fixed)?,
                    &mut added,
                    &mut removed,
                    &mut parts,
                )?;
            }
            Ty::Map(_, _) if name == "remove" => {
                for value in members(expr, &mut fixed)? {
                    let Literal::Int(key) = value else {
                        return Err("delta map removal requires constant i64 keys".into());
                    };
                    if !removed.insert(key) {
                        return Err("duplicate delta map key".into());
                    }
                    parts.push(Part::Removed(Literal::Int(key)));
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
    if added.intersection(&removed).next().is_some() {
        return Err("delta additions/upserts overlap removals".into());
    }
    Ok(parts)
}
fn member_key(value: &Literal) -> Result<i64, String> {
    match value {
        Literal::Int(value) => Ok(*value),
        Literal::Bool(value) => Ok(i64::from(*value)),
        Literal::Float(_)
        | Literal::Str(_)
        | Literal::CivilDateTime(_)
        | Literal::TimeZone(_)
        | Literal::ZonedTime(_)
        | Literal::ZonedDateTime(_)
        | Literal::Duration(_)
        | Literal::Date(_)
        | Literal::Time(_)
        | Literal::DateTime(_) => Err("delta set member requires bool or i64".into()),
    }
}
fn members(
    expr: &Expr,
    fixed: &mut impl FnMut(&Expr) -> Result<Literal, String>,
) -> Result<Vec<Literal>, String> {
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
    fixed: &mut impl FnMut(&Expr) -> Result<Literal, String>,
    supplied: &mut BTreeSet<i64>,
    parts: &mut Vec<Part<'a>>,
) -> Result<(), String> {
    if matches!(expr, Expr::Sequence(values) if values.is_empty()) {
        return Ok(());
    }
    let Expr::Sparse(entries) = expr else {
        return Err("delta child entries require sparse key:payload syntax".into());
    };
    for (key, payload) in entries {
        let Literal::Int(key) = fixed(key)? else {
            return Err("delta index/key requires constant i64".into());
        };
        if !supplied.insert(key) {
            return Err("duplicate delta index/key".into());
        }
        let child = match origin {
            Ty::Map(_, child) => child.as_ref(),
            Ty::List(child, Some(size))
                if usize::try_from(key).is_ok_and(|index| index < *size) =>
            {
                child.as_ref()
            }
            Ty::Tuple(children) => children
                .get(usize::try_from(key).map_err(|_range| "delta index out of bounds")?)
                .ok_or("delta index out of bounds")?,
            Ty::Delta(_)
            | Ty::List(..)
            | Ty::Struct(..)
            | Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Str
            | Ty::CivilDateTime
            | Ty::TimeZone
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
        parts.push(Part::Child(key, child.clone().delta()?, payload));
    }
    Ok(())
}

fn set_members(
    member: &Ty,
    name: &str,
    values: Vec<Literal>,
    added: &mut BTreeSet<i64>,
    removed: &mut BTreeSet<i64>,
    parts: &mut Vec<Part<'_>>,
) -> Result<(), String> {
    for value in values {
        if value.ty() != *member {
            return Err("delta set member type mismatch".into());
        }
        let key = member_key(&value)?;
        let keys = if name == "added" {
            &mut *added
        } else {
            &mut *removed
        };
        if !keys.insert(key) {
            return Err("duplicate delta set member".into());
        }
        parts.push(if name == "added" {
            Part::Added(value)
        } else {
            Part::Removed(value)
        });
    }

    Ok(())
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
