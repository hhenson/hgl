//! Recursive execution of compiled owning descriptors; no dynamic type registry.
use hgl_types::any_values::{AnyExecution, AnyView, Frames, index, number};
use hgl_types::{AnyOperation, AnyPlan, NodeError, NodeResult};
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};
/// Operations bound directly into generated descriptors.
pub static EXECUTION: AnyExecution = AnyExecution {
    equal,
    compare,
    hash,
};
fn capability<T>() -> NodeResult<T> {
    Err(NodeError::coded(
        "contained value lacks the required capability",
        "value.capability",
    ))
}
/// Delegate equality through the precompiled descendant layout.
fn equal(plan: &AnyPlan, a: AnyView<'_>, b: AnyView<'_>) -> NodeResult<bool> {
    if !plan.capabilities[0] {
        return capability();
    }
    match &plan.operation {
        AnyOperation::Native { equal, .. } => equal(&a, &b),
        AnyOperation::Float => Ok(f64::from_bits(number(&a))
            .partial_cmp(&f64::from_bits(number(&b)))
            == Some(Ordering::Equal)),
        AnyOperation::Integer | AnyOperation::Octets => Ok(*a == *b),
        AnyOperation::Fields(fields) => {
            for ((left, right), field) in Frames(a).zip(Frames(b)).zip(*fields) {
                if !left.field(field).equal(&right.field(field))? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        AnyOperation::Sequence(child, unordered) => {
            Frames(a).equal_by(Frames(b), *unordered, |left, right| {
                child.equal(left, right)
            })
        }
        AnyOperation::Result(ok, err) => {
            if a[0] != b[0] {
                return Ok(false);
            }
            let child = if a[0] == 0 { ok } else { err };
            child.equal(a.slice(9..a.len()), b.slice(9..b.len()))
        }
        AnyOperation::Error(text) => {
            let (mut left, mut right) = (Frames(a), Frames(b));
            if !text.equal(
                left.next()
                    .unwrap_or_else(|| unreachable!("validated diagnostic message")),
                right
                    .next()
                    .unwrap_or_else(|| unreachable!("validated diagnostic message")),
            )? {
                return Ok(false);
            }
            let code = hgl_types::AnyField {
                plan: text,
                optional: true,
            };
            left.next()
                .unwrap_or_else(|| unreachable!("validated diagnostic code"))
                .field(&code)
                .equal(
                    &right
                        .next()
                        .unwrap_or_else(|| unreachable!("validated diagnostic code"))
                        .field(&code),
                )
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
/// Return partial order without type-name ordering or numeric coercion.
fn compare(plan: &AnyPlan, a: AnyView<'_>, b: AnyView<'_>) -> NodeResult<Option<Ordering>> {
    if !plan.capabilities[2] {
        return capability();
    }
    match &plan.operation {
        AnyOperation::Native { compare, .. } => compare(&a, &b),
        AnyOperation::Integer => Ok(Some(
            i64::from_le_bytes(number(&a).to_le_bytes())
                .cmp(&i64::from_le_bytes(number(&b).to_le_bytes())),
        )),
        AnyOperation::Float => {
            Ok(f64::from_bits(number(&a)).partial_cmp(&f64::from_bits(number(&b))))
        }
        AnyOperation::Octets => Ok(Some(a[..].cmp(&b[..]))),
        AnyOperation::Fields(fields) => Frames(a).compare_by(Frames(b), |i, left, right| {
            left.field(&fields[i]).compare(&right.field(&fields[i]))
        }),
        AnyOperation::Sequence(child, false) => {
            Frames(a).compare_by(Frames(b), |_, left, right| child.compare(left, right))
        }
        AnyOperation::Family(members) => {
            let (left, right) = (number(&a[..8]), number(&b[..8]));
            if left != right {
                return Ok(None);
            }
            members[index(left)].compare(a.slice(8..a.len()), b.slice(8..b.len()))
        }
        AnyOperation::Boxed => a.boxed().compare(&b.boxed()),
        AnyOperation::Sequence(_, true)
        | AnyOperation::Result(..)
        | AnyOperation::Error(_)
        | AnyOperation::Data => capability(),
    }
}
/// Hash complete contents; unordered containers combine member hashes independently.
fn hash(plan: &AnyPlan, data: AnyView<'_>) -> NodeResult<u64> {
    if !plan.capabilities[0] || !plan.capabilities[1] {
        return capability();
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    match &plan.operation {
        AnyOperation::Native { hash, .. } => return hash(&data),
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
                data.field(field).key_hash()?.hash(&mut hash);
            }
        }
        AnyOperation::Sequence(child, unordered) => {
            let mut members = Frames(data).map(|data| child.hash(data));
            if *unordered {
                members
                    .try_fold(0, |combined, member| {
                        Ok::<_, Box<NodeError>>(combined ^ member?)
                    })?
                    .hash(&mut hash);
            } else {
                for member in members {
                    member?.hash(&mut hash);
                }
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
        AnyOperation::Data | AnyOperation::Result(..) | AnyOperation::Error(_) => {
            return capability();
        }
    }
    Ok(hash.finish())
}
