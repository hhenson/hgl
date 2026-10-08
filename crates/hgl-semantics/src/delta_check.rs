//! Check sparse ordinary delta formation without evaluating payload expressions.
use crate::composite_keys::{Key, key};
use crate::ir::{DeltaEntry, Kind, Value};
use hgl_source::{Expr, Issue, Literal, Ty};
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
fn error(code: &'static str, expr: &Expr, message: impl Into<String>) -> Issue {
    Issue::coded("type", code, expr.span(), message)
}
/// Validate a complete constructor before checking or evaluating payloads.
pub fn constructor<'a>(
    origin: &Ty,
    args: &'a [(Option<String>, Expr)],
    mut fixed: impl FnMut(&Expr, &Ty) -> Result<(Value, Option<Value>), Issue>,
) -> Result<Vec<Part<'a>>, Issue> {
    if !origin.publication()
        || !matches!(
            origin,
            Ty::Set(_) | Ty::Map(..) | Ty::List(..) | Ty::Tuple(_) | Ty::Struct(..)
        )
    {
        return Err(Issue::coded(
            "shape",
            "delta.unsupported_shape",
            0..0,
            "delta constructor requires a supported structural publication shape",
        ));
    }
    let mut names = BTreeSet::new();
    let mut added = BTreeSet::new();
    let mut removed = BTreeSet::new();
    let mut parts = Vec::new();
    for (name, expr) in args {
        let name = name
            .as_deref()
            .ok_or("delta constructor arguments must be named")?;
        if !names.insert(name) {
            return Err(Issue::coded(
                "name",
                "delta.duplicate_argument",
                expr.argument_span(),
                format!("duplicate delta argument {name}"),
            ));
        }
        let member = match (origin, name) {
            (Ty::Set(member), "added" | "removed") | (Ty::Map(member, _), "remove") => {
                Some(member.as_ref())
            }
            (Ty::List(_, None), "remove") => Some(&Ty::I64),
            _ => None,
        };
        if let Some(member) = member {
            members(
                (origin, member),
                name,
                expr,
                &mut fixed,
                (&mut added, &mut removed),
                &mut parts,
            )?;
        } else if matches!(
            (origin, name),
            (Ty::Map(..), "upsert") | (Ty::List(..) | Ty::Tuple(_), "items")
        ) {
            sparse(origin, expr, &mut fixed, &mut added, &removed, &mut parts)?;
        } else if let Ty::Struct(_, fields, _) = origin {
            let (index, (_, child)) = fields
                .iter()
                .enumerate()
                .find(|(_, (field, _))| field == name)
                .ok_or_else(|| {
                    Issue::coded(
                        "name",
                        "delta.argument_name",
                        expr.argument_span(),
                        format!("unknown delta field {name}"),
                    )
                })?;
            parts.push(Part::Child(
                i64::try_from(index).map_err(|e| e.to_string())?,
                child.clone().delta()?,
                expr,
            ));
        } else {
            return Err(Issue::coded(
                "name",
                "delta.argument_name",
                expr.argument_span(),
                format!("unknown delta argument {name}"),
            ));
        }
    }
    Ok(parts)
}
fn members<'a>(
    (origin, member): (&Ty, &Ty),
    name: &str,
    expr: &'a Expr,
    fixed: &mut impl FnMut(&Expr, &Ty) -> Result<(Value, Option<Value>), Issue>,
    (added, removed): (&mut BTreeSet<Key>, &mut BTreeSet<Key>),
    parts: &mut Vec<Part<'a>>,
) -> Result<(), Issue> {
    let Expr::Sequence(values) = expr.syntax() else {
        return Err(error(
            "delta.entry_constant",
            expr,
            "delta membership/removal requires a constant literal list",
        ));
    };
    for expression in values {
        let expression = expression.as_ref().ok_or("delta member cannot be absent")?;
        let value = fixed(expression, member)?;
        let keys = if name == "added" {
            &mut *added
        } else {
            &mut *removed
        };
        remember(&value, member, keys, expression)?;
        if matches!(origin, Ty::List(..)) {
            index(&value, expression)?;
        }
        if !matches!(origin, Ty::List(..)) {
            overlap(added, removed, expression)?;
        }
        parts.push(if name == "added" {
            Part::Added(value.0)
        } else {
            Part::Removed(value.0)
        });
    }
    Ok(())
}
fn remember(
    value: &(Value, Option<Value>),
    expected: &Ty,
    keys: &mut BTreeSet<Key>,
    expr: &Expr,
) -> Result<(), Issue> {
    if value.0.ty != *expected {
        return Err(error(
            "delta.entry_type",
            expr,
            format!(
                "delta entry type mismatch: requires constant {}",
                expected.source_name()
            ),
        ));
    }
    if let Some(value) = &value.1
        && !keys.insert(key(value)?)
    {
        return Err(error(
            "delta.duplicate_entry",
            expr,
            "duplicate delta entry",
        ));
    }
    Ok(())
}
fn overlap(added: &BTreeSet<Key>, removed: &BTreeSet<Key>, expr: &Expr) -> Result<(), Issue> {
    if added.intersection(removed).next().is_some() {
        return Err(error(
            "delta.overlap",
            expr,
            "delta additions/upserts overlap removals",
        ));
    }
    Ok(())
}
fn index(value: &(Value, Option<Value>), expr: &Expr) -> Result<i64, Issue> {
    let Some(Value {
        kind: Kind::Literal(Literal::Int(index)),
        ..
    }) = &value.1
    else {
        return Err(error(
            "delta.entry_constant",
            expr,
            "delta index requires constant i64",
        ));
    };
    if *index < 0 {
        return Err(error(
            "delta.index_bounds",
            expr,
            "delta index out of bounds: requires nonnegative index",
        ));
    }
    Ok(*index)
}
fn sparse<'a>(
    origin: &Ty,
    expr: &'a Expr,
    fixed: &mut impl FnMut(&Expr, &Ty) -> Result<(Value, Option<Value>), Issue>,
    added: &mut BTreeSet<Key>,
    removed: &BTreeSet<Key>,
    parts: &mut Vec<Part<'a>>,
) -> Result<(), Issue> {
    if matches!(expr.syntax(), Expr::Sequence(values) if values.is_empty()) {
        return Ok(());
    }
    let Expr::Sparse(entries) = expr.syntax() else {
        return Err("delta child entries require sparse key:payload syntax".into());
    };
    for (position, payload) in entries {
        let expected = if let Ty::Map(member, _) = origin {
            member.as_ref()
        } else {
            &Ty::I64
        };
        let value = fixed(position, expected)?;
        remember(&value, expected, added, position)?;
        if let Ty::Map(_, child) = origin {
            overlap(added, removed, position)?;
            parts.push(Part::Keyed(value.0, child.clone().delta()?, payload));
        } else {
            let index = index(&value, position)?;
            let child = if let Ty::List(child, size) = origin {
                size.is_none_or(|size| usize::try_from(index).is_ok_and(|index| index < size))
                    .then_some(child.as_ref())
            } else if let Ty::Tuple(children) = origin {
                usize::try_from(index)
                    .ok()
                    .and_then(|index| children.get(index))
            } else {
                None
            }
            .ok_or_else(|| error("delta.index_bounds", position, "delta index out of bounds"))?;
            parts.push(Part::Child(index, child.clone().delta()?, payload));
        }
    }
    Ok(())
}
/// Check payloads and retain every complete key recipe in its written position.
pub fn values(
    parts: Vec<Part<'_>>,
    mut check: impl FnMut(&Ty, &Expr) -> Result<Value, Issue>,
) -> Result<Vec<DeltaEntry>, Issue> {
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
    if added.intersection(&removed).next().is_some() {
        return Err("delta additions/upserts overlap removals".into());
    }
    Ok(())
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

/// Lower the eval-only positional sparse shorthand under an exact tuple origin.
pub fn tuple_shorthand(
    cells: &[Option<Expr>],
    expected: &Ty,
    mut check: impl FnMut(&Expr, &Ty) -> Result<Value, Issue>,
) -> Result<Value, Issue> {
    let Ty::Delta(origin) = expected else {
        return Err("tuple shorthand requires an exact tuple publication shape".into());
    };
    let Ty::Tuple(children) = origin.as_ref() else {
        return Err("tuple shorthand requires an exact tuple publication shape".into());
    };
    if children.len() != cells.len() {
        return Err("tuple shorthand arity mismatch".into());
    }
    let mut entries = Vec::new();
    for (index, (child, expr)) in children.iter().zip(cells).enumerate() {
        if let Some(expr) = expr {
            let ty = child.clone().delta()?;
            let mut value = check(expr, &ty)?;
            if ty == Ty::F64 && value.ty == Ty::I64 {
                value = Value::new(Ty::F64, Kind::Unary("float".into(), Box::new(value)));
            }
            if value.ty != ty {
                return Err(Issue::coded(
                    "type",
                    "delta.type_mismatch",
                    expr.span(),
                    "tuple shorthand child type mismatch",
                ));
            }
            entries.push(DeltaEntry::Child(
                i64::try_from(index).map_err(|e| e.to_string())?,
                value,
            ));
        }
    }
    Ok(Value::new(expected.clone(), Kind::Delta(entries)))
}
