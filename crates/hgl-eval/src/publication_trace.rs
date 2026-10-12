//! Validate canonical closed publication sequences before graph start.
use crate::composite_keys::Key;
use crate::ir::{DeltaEntry, Kind, Value};
use crate::key_context::Context;
use crate::value_eval::EvalError;
use hgl_value_model::{Literal, Ty};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
struct State {
    members: BTreeSet<Key>,
    children: BTreeMap<Key, Self>,
}
/// Validate sparse publications from a fresh endpoint, preserving prior membership.
pub fn validate(shape: &Ty, slots: &[Option<Value>]) -> Result<(), (usize, String)> {
    let mut state = State::default();
    let mut identities = Context::default();
    for (index, value) in slots.iter().enumerate() {
        if let Some(value) = value {
            state
                .apply(shape, value, &mut identities)
                .map_err(|error| (index, error.to_string()))?;
        }
    }
    Ok(())
}
impl State {
    fn growing(&mut self, parts: &[DeltaEntry]) -> Result<(), String> {
        let items = parts.iter().filter_map(|part| {
            if let DeltaEntry::Child(index, _) = part {
                Some(*index)
            } else {
                None
            }
        });
        let removed = parts.iter().filter_map(|part| {
            if let DeltaEntry::Remove(Value {
                kind: Kind::Literal(Literal::Int(index)),
                ..
            }) = part
            {
                Some(*index)
            } else {
                None
            }
        });
        hgl_types::growing_range::validate(self.children.len(), items, removed.clone())
            .map_err(str::to_owned)?;
        for index in removed {
            self.children
                .remove(&crate::composite_keys::scalar(&Literal::Int(index))?);
        }
        Ok(())
    }
    fn apply(
        &mut self,
        shape: &Ty,
        value: &Value,
        identities: &mut Context,
    ) -> Result<(), EvalError> {
        let Kind::Delta(parts) = &value.kind else {
            return Ok(());
        };
        materialized(parts)?;
        if matches!(shape, Ty::List(_, None)) {
            self.growing(parts)?;
        }
        for part in parts {
            match (shape, part) {
                (Ty::List(_, None), DeltaEntry::Remove(_)) => {}
                (Ty::Set(_), DeltaEntry::Add(member)) => {
                    if !self.members.insert(identities.key(member)?) {
                        return Err("set addition is already present".into());
                    }
                }
                (Ty::Set(_), DeltaEntry::Remove(member)) => {
                    if !self.members.remove(&identities.key(member)?) {
                        return Err("set removal is absent".into());
                    }
                }
                (Ty::Map(..), DeltaEntry::Remove(member)) => {
                    if self.children.remove(&identities.key(member)?).is_none() {
                        return Err("map removal is absent".into());
                    }
                }
                (Ty::Map(_, child), DeltaEntry::Keyed(member, value)) => self
                    .children
                    .entry(identities.key(member)?)
                    .or_default()
                    .apply(child, value, identities)?,
                (_, DeltaEntry::Child(index, value)) => {
                    let position = usize::try_from(*index).map_err(|e| e.to_string())?;
                    let child = if let Ty::List(child, _) = shape {
                        Some(child.as_ref())
                    } else if let Ty::Tuple(children) = shape {
                        children.get(position)
                    } else {
                        shape
                            .structure()
                            .ok()
                            .and_then(|(_, fields, _)| fields.get(position))
                            .map(|(_, ty)| ty)
                    }
                    .ok_or("delta does not match its checked publication shape")?;
                    self.children
                        .entry(crate::composite_keys::scalar(&Literal::Int(*index))?)
                        .or_default()
                        .apply(child, value, identities)?;
                }
                _ => return Err("delta does not match its checked publication shape".into()),
            }
        }
        Ok(())
    }
}

/// Validate exact duplicate and overlap identities after cold materialization.
pub fn materialized(parts: &[DeltaEntry]) -> Result<(), EvalError> {
    let mut added = BTreeSet::new();
    let mut identities = Context::default();
    let mut removed = BTreeSet::new();
    for part in parts {
        let (value, keys) = match part {
            DeltaEntry::Add(value) | DeltaEntry::Keyed(value, _) => (value, &mut added),
            DeltaEntry::Remove(value) => (value, &mut removed),
            DeltaEntry::Child(..) => continue,
        };
        if !keys.insert(identities.key(value)?) {
            return Err("duplicate delta key/member".into());
        }
    }
    if added.intersection(&removed).next().is_some() {
        return Err("delta additions/upserts overlap removals".into());
    }
    Ok(())
}
