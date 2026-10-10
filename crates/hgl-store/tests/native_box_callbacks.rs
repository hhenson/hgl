//! Opaque boxed values delegate operations to preselected provider callbacks.
use hgl_types::{AnyOperation, AnyPlan, AnyValue, NodeError, NodeResult};
use std::{any::TypeId, cmp::Ordering};
#[derive(Debug)]
struct Opaque;
fn identity() -> TypeId {
    TypeId::of::<Opaque>()
}
fn datum(data: &[u8]) -> NodeResult<u8> {
    data.first()
        .copied()
        .ok_or_else(|| NodeError::new("invalid provider encoding"))
}
fn equal(a: &[u8], b: &[u8]) -> NodeResult<bool> {
    Ok(datum(a)? == datum(b)?)
}
fn hash(data: &[u8]) -> NodeResult<u64> {
    Ok(u64::from(datum(data)?))
}
fn compare(a: &[u8], b: &[u8]) -> NodeResult<Option<Ordering>> {
    Ok(Some(datum(a)?.cmp(&datum(b)?)))
}
static PLAN: AnyPlan = AnyPlan {
    index: 0,
    identity,
    operation: AnyOperation::Native {
        equal,
        hash,
        compare,
    },
    capabilities: [true; 3],
};
static RESTRICTED: AnyPlan = AnyPlan {
    index: 0,
    identity,
    operation: AnyOperation::Native {
        equal,
        hash,
        compare,
    },
    capabilities: [false; 3],
};
#[test]
fn opaque_bytes_are_never_engine_equality_or_ordering() -> NodeResult {
    let a = AnyValue::from_encoded(&PLAN, vec![3, 0]);
    let b = AnyValue::from_encoded(&PLAN, vec![3, 255]);
    let c = AnyValue::from_encoded(&PLAN, vec![4, 0]);
    let (result, allocations) = hgl_alloc_count::count_in(|| -> NodeResult {
        assert!(a.equal(&b)?);
        assert_eq!(a.key_hash()?, b.key_hash()?);
        assert_eq!(a.compare(&b)?, Some(Ordering::Equal));
        assert_eq!(a.compare(&c)?, Some(Ordering::Less));
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    Ok(())
}
#[test]
fn checked_capability_mask_hides_extra_physical_operations() {
    let a = AnyValue::from_encoded(&RESTRICTED, vec![3, 0]);
    let b = AnyValue::from_encoded(&RESTRICTED, vec![3, 255]);
    for result in [
        a.equal(&b).map(|_| ()),
        a.key_hash().map(|_| ()),
        a.compare(&b).map(|_| ()),
    ] {
        assert!(matches!(result, Err(error) if error.code == Some("value.capability")));
    }
}
