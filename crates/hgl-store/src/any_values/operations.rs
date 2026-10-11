//! Recursive execution of compiled owning descriptors; no dynamic type registry.
use hgl_types::any_values::{AnyExecution, AnyRef, AnyView, Frames};
use hgl_types::{AnyField, AnyOperation, AnyPlan, NodeError, NodeResult};
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};
/// Operations bound directly into generated descriptors.
pub static EXECUTION: AnyExecution = AnyExecution {
    equal: |plan, a, b| Operations(plan).equal(a, b),
    compare: |plan, a, b| Operations(plan).compare(a, b),
    hash: |plan, data| Operations(plan).hash(data),
};
fn number(data: &[u8]) -> u64 {
    u64::from_le_bytes(
        data.try_into()
            .unwrap_or_else(|_| unreachable!("compiled scalar layout")),
    )
}
fn index(value: u64) -> usize {
    usize::try_from(value).unwrap_or_else(|_| unreachable!("compiled descriptor index"))
}
fn capability<T>() -> NodeResult<T> {
    Err(NodeError::coded(
        "contained value lacks the required capability",
        "value.capability",
    ))
}
struct Operations<'a>(&'a AnyPlan);
impl std::ops::Deref for Operations<'_> {
    type Target = AnyPlan;
    fn deref(&self) -> &AnyPlan {
        self.0
    }
}
impl Operations<'_> {
    /// Delegate equality through the precompiled descendant layout.
    fn equal(&self, a: AnyView<'_>, b: AnyView<'_>) -> NodeResult<bool> {
        if !self.capabilities[0] {
            return capability();
        }
        match &self.operation {
            AnyOperation::Float => Ok(f64::from_bits(number(&a))
                .partial_cmp(&f64::from_bits(number(&b)))
                == Some(Ordering::Equal)),
            AnyOperation::Integer | AnyOperation::Octets => Ok(*a == *b),
            AnyOperation::Fields(fields) => {
                for ((left, right), field) in Frames(a).zip(Frames(b)).zip(*fields) {
                    if !field_value(field, left).equal(&field_value(field, right))? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            AnyOperation::Sequence(child, unordered) => {
                Operations(child).sequence_equal(a, b, *unordered)
            }
            AnyOperation::Family(members) => {
                let (left, right) = (number(&a[..8]), number(&b[..8]));
                Ok(left == right
                    && members[index(left)].equal(a.slice(8..a.len()), b.slice(8..b.len()))?)
            }
            AnyOperation::Boxed => a.boxed().equal(&b.boxed()),
            AnyOperation::Data => capability(),
        }
    }
    fn sequence_equal(&self, a: AnyView<'_>, b: AnyView<'_>, unordered: bool) -> NodeResult<bool> {
        if Frames(a).count() != Frames(b).count() {
            return Ok(false);
        }
        if unordered {
            for left in Frames(a) {
                if !Frames(b).try_fold(false, |found, right| -> NodeResult<bool> {
                    Ok(found || self.equal(left, right)?)
                })? {
                    return Ok(false);
                }
            }
        } else {
            for (left, right) in Frames(a).zip(Frames(b)) {
                if !self.equal(left, right)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    /// Return partial order without type-name ordering or numeric coercion.
    fn compare(&self, a: AnyView<'_>, b: AnyView<'_>) -> NodeResult<Option<Ordering>> {
        if !self.capabilities[2] {
            return capability();
        }
        match &self.operation {
            AnyOperation::Integer => Ok(Some(
                i64::from_le_bytes(number(&a).to_le_bytes())
                    .cmp(&i64::from_le_bytes(number(&b).to_le_bytes())),
            )),
            AnyOperation::Float => {
                Ok(f64::from_bits(number(&a)).partial_cmp(&f64::from_bits(number(&b))))
            }
            AnyOperation::Octets => Ok(Some(a[..].cmp(&b[..]))),
            AnyOperation::Fields(fields) => {
                for ((left, right), field) in Frames(a).zip(Frames(b)).zip(*fields) {
                    let order = field_value(field, left).compare(&field_value(field, right))?;
                    if order != Some(Ordering::Equal) {
                        return Ok(order);
                    }
                }
                Ok(Some(Ordering::Equal))
            }
            AnyOperation::Sequence(child, false) => {
                for (left, right) in Frames(a).zip(Frames(b)) {
                    let order = child.compare(left, right)?;
                    if order != Some(Ordering::Equal) {
                        return Ok(order);
                    }
                }
                Ok(Some(Frames(a).count().cmp(&Frames(b).count())))
            }
            AnyOperation::Family(members) => {
                let (left, right) = (number(&a[..8]), number(&b[..8]));
                if left != right {
                    return Ok(None);
                }
                members[index(left)].compare(a.slice(8..a.len()), b.slice(8..b.len()))
            }
            AnyOperation::Boxed => a.boxed().compare(&b.boxed()),
            AnyOperation::Sequence(_, true) | AnyOperation::Data => capability(),
        }
    }
    /// Hash complete contents; unordered containers combine member hashes independently.
    fn hash(&self, data: AnyView<'_>) -> NodeResult<u64> {
        if !self.capabilities[0] || !self.capabilities[1] {
            return capability();
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        match &self.operation {
            AnyOperation::Float => {
                let value = f64::from_bits(number(&data));
                if value.is_nan() {
                    return Err(NodeError::new(
                        "NaN collection keys and members are unsupported",
                    ));
                }
                (if value == 0.0 { 0 } else { value.to_bits() }).hash(&mut hash);
            }
            AnyOperation::Integer | AnyOperation::Octets => data[..].hash(&mut hash),
            AnyOperation::Fields(fields) => {
                for (field, data) in fields.iter().zip(Frames(data)) {
                    field_value(field, data).key_hash()?.hash(&mut hash);
                }
            }
            AnyOperation::Sequence(child, unordered) => {
                let mut combined = 0;
                for data in Frames(data) {
                    let member = child.hash(data)?;
                    if *unordered {
                        combined ^= member;
                    } else {
                        member.hash(&mut hash);
                    }
                }
                if *unordered {
                    combined.hash(&mut hash);
                }
            }
            AnyOperation::Family(members) => {
                let member = number(&data[..8]);
                member.hash(&mut hash);
                members[index(member)]
                    .hash(data.slice(8..data.len()))?
                    .hash(&mut hash);
            }
            AnyOperation::Boxed => data.boxed().key_hash()?.hash(&mut hash),
            AnyOperation::Data => return capability(),
        }
        Ok(hash.finish())
    }
}
fn field_value<'a>(field: &AnyField, data: AnyView<'a>) -> AnyRef<'a> {
    let present = !field.optional || data[0] != 0;
    let payload = if field.optional {
        data.slice(1..data.len())
    } else {
        data
    };
    AnyRef {
        plan: present.then_some(field.plan),
        data: payload,
    }
}
