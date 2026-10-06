//! Validate canonical closed publication sequences before graph start.
use crate::composite_keys::{Key, key};
use crate::ir::{DeltaEntry, Kind, Value};
use hgl_source::{Literal, Ty};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
struct State {
    members: BTreeSet<Key>,
    children: BTreeMap<Key, Self>,
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
    fn apply(&mut self, shape: &Ty, value: &Value) -> Result<(), String> {
        let Kind::Delta(parts) = &value.kind else {
            return Ok(());
        };
        if parts.is_empty() {
            return Err("empty structural publication".into());
        }
        crate::delta_check::materialized(parts)?;
        if matches!(shape, Ty::List(_, None)) {
            self.growing(parts)?;
        }
        for part in parts {
            match (shape, part) {
                (Ty::List(_, None), DeltaEntry::Remove(_)) => {}
                (Ty::Set(_), DeltaEntry::Add(member)) => {
                    if !self.members.insert(key(member)?) {
                        return Err("set addition is already present".into());
                    }
                }
                (Ty::Set(_), DeltaEntry::Remove(member)) => {
                    if !self.members.remove(&key(member)?) {
                        return Err("set removal is absent".into());
                    }
                }
                (Ty::Map(..), DeltaEntry::Remove(member)) => {
                    if self.children.remove(&key(member)?).is_none() {
                        return Err("map removal is absent".into());
                    }
                }
                (Ty::Map(_, child), DeltaEntry::Keyed(member, value)) => self
                    .children
                    .entry(key(member)?)
                    .or_default()
                    .apply(child, value)?,
                (Ty::List(child, _), DeltaEntry::Child(index, value)) => self
                    .children
                    .entry(crate::composite_keys::scalar(&Literal::Int(*index))?)
                    .or_default()
                    .apply(child, value)?,
                (Ty::Tuple(children), DeltaEntry::Child(index, value)) => {
                    let index = usize::try_from(*index).map_err(|e| e.to_string())?;
                    self.children
                        .entry(crate::composite_keys::scalar(&Literal::Int(
                            i64::try_from(index).map_err(|e| e.to_string())?,
                        ))?)
                        .or_default()
                        .apply(&children[index], value)?;
                }
                (Ty::Struct(_, fields, _), DeltaEntry::Child(index, value)) => {
                    let position = usize::try_from(*index).map_err(|e| e.to_string())?;
                    self.children
                        .entry(crate::composite_keys::scalar(&Literal::Int(*index))?)
                        .or_default()
                        .apply(&fields[position].1, value)?;
                }
                _ => return Err("delta does not match its checked publication shape".into()),
            }
        }
        Ok(())
    }
}
