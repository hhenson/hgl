//! Closed ordinary replay data and pre-start publication admission.
use hgl_rust_ir::{DeltaEntry, Kind, Value};
use hgl_source::{Expr, Literal, Ty};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
struct State {
    members: BTreeSet<i64>,
    children: BTreeMap<i64, Self>,
}
/// Validate sparse publications from a fresh endpoint, preserving prior membership.
pub fn validate(shape: &Ty, slots: &[Option<Value>]) -> Result<(), (usize, String)> {
    let mut state = State::default();
    for (index, value) in slots.iter().enumerate() {
        if let Some(value) = value {
            state.apply(shape, value).map_err(|error| (index, error))?;
        }
    }
    Ok(())
}
impl State {
    fn apply(&mut self, shape: &Ty, value: &Value) -> Result<(), String> {
        let Kind::Delta(parts) = &value.kind else {
            return Ok(());
        };
        if parts.is_empty() {
            return Err("empty structural publication".into());
        }
        for part in parts {
            match (shape, part) {
                (Ty::Set(_), DeltaEntry::Add(member)) => {
                    if !self.members.insert(key(member)) {
                        return Err("set addition is already present".into());
                    }
                }
                (Ty::Set(_), DeltaEntry::Remove(member)) => {
                    if !self.members.remove(&key(member)) {
                        return Err("set removal is absent".into());
                    }
                }
                (Ty::Map(..), DeltaEntry::Remove(member)) => {
                    if self.children.remove(&key(member)).is_none() {
                        return Err("map removal is absent".into());
                    }
                }
                (Ty::Map(_, child) | Ty::List(child, _), DeltaEntry::Child(index, value)) => self
                    .children
                    .entry(*index)
                    .or_default()
                    .apply(child, value)?,
                (Ty::Tuple(children), DeltaEntry::Child(index, value)) => {
                    let index = usize::try_from(*index).map_err(|e| e.to_string())?;
                    self.children
                        .entry(i64::try_from(index).map_err(|e| e.to_string())?)
                        .or_default()
                        .apply(&children[index], value)?;
                }
                (Ty::Struct(_, fields), DeltaEntry::Child(index, value)) => {
                    let position = usize::try_from(*index).map_err(|e| e.to_string())?;
                    self.children
                        .entry(*index)
                        .or_default()
                        .apply(&fields[position].1, value)?;
                }
                _ => return Err("delta does not match its checked publication shape".into()),
            }
        }
        Ok(())
    }
}
fn key(value: &Literal) -> i64 {
    match value {
        Literal::Int(value) => *value,
        Literal::Bool(value) => i64::from(*value),
        Literal::Float(_)
        | Literal::Str(_)
        | Literal::Date(_)
        | Literal::Time(_)
        | Literal::DateTime(_)
        | Literal::Duration(_) => unreachable!("checked set member or map key"),
    }
}
/// Turn present dense slots into owned ordinary timed entries; absence adds no data.
pub fn timed(entry_type: Ty, slots: &[Option<Value>]) -> Result<Value, String> {
    let mut entries = Vec::new();
    for (index, value) in slots.iter().enumerate() {
        if let Some(value) = value {
            let micros = i64::try_from(index)
                .map_err(|error| format!("eval timestamp overflow: {error}"))?
                .checked_add(1)
                .ok_or("eval timestamp overflow")?;
            entries.push(Value::new(
                entry_type.clone(),
                Kind::Construct(vec![
                    (
                        0,
                        Value::new(Ty::DateTime, Kind::Literal(Literal::DateTime(micros))),
                    ),
                    (1, value.clone()),
                ]),
            ));
        }
    }
    Ok(Value::new(
        Ty::List(Box::new(entry_type), None),
        Kind::List(entries),
    ))
}

/// Contextually check dense input cells without turning silence into a value.
pub fn sequence(
    ticks: &[Option<Expr>],
    mut ty: Option<Ty>,
    mut check: impl FnMut(&Expr, Option<&Ty>) -> Result<Value, String>,
) -> Result<(Ty, Vec<Option<Value>>), String> {
    let mut slots = Vec::new();
    for expr in ticks {
        let value = if let Some(expr) = expr {
            let expected = ty.clone().map(Ty::delta).transpose()?;
            let value = check(expr, expected.as_ref())?;
            ty.get_or_insert_with(|| {
                if let Ty::Delta(origin) = &value.ty {
                    *origin.clone()
                } else {
                    value.ty.clone()
                }
            });
            Some(value)
        } else {
            None
        };
        slots.push(value);
    }
    let ty = ty.ok_or("cannot infer empty generic sequence")?;
    if !ty.publication() {
        return Err("eval input is outside the delta publication profile".into());
    }
    Ok((ty, slots))
}
