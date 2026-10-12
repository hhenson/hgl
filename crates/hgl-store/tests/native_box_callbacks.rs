//! Opaque boxed values delegate operations to preselected provider callbacks.
use hgl_types::any_values::{AnyBuffer, AnyWidth, Frames};
use hgl_types::{AnyOperation, AnyPlan, AnyValue, NodeError, NodeResult};
use std::{
    any::TypeId,
    cmp::Ordering,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};
static COPIES: AtomicUsize = AtomicUsize::new(0);
fn copy(data: &[u8], out: &mut dyn AnyBuffer) {
    COPIES.fetch_add(1, Relaxed);
    if let Some((first, rest)) = data.split_first() {
        out.write(&[*first]);
        for byte in rest {
            out.write(&[byte.wrapping_add(1)]);
        }
    }
}
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
    execution: &hgl_store::any_values::operations::EXECUTION,
    capture: None,
    validate: None,
    identity,
    copy: |data, out| out.native(&data, copy, |_, _| Ok(())),
    operation: AnyOperation::Native {
        equal,
        hash,
        compare,
    },
    capabilities: [true; 3],
};
static RESTRICTED: AnyPlan = AnyPlan {
    execution: &hgl_store::any_values::operations::EXECUTION,
    capture: None,
    validate: None,
    identity,
    copy: |data, out| out.native(&data, copy, |_, _| Ok(())),
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

static NESTED: AnyPlan = AnyPlan {
    execution: &hgl_store::any_values::operations::EXECUTION,
    capture: None,
    validate: None,
    identity: TypeId::of::<Vec<Opaque>>,
    operation: AnyOperation::Sequence(&PLAN, false),
    copy: |data, out| {
        for data in Frames(data) {
            let start = out.len();
            out.write(&[0; 8]);
            (PLAN.copy)(data, out);
            out.width(start, out.len() - start - 8);
        }
    },
    capabilities: [true; 3],
};
#[test]
fn nested_copy_sizing_and_retention_preserve_independent_aliases() -> NodeResult {
    COPIES.store(0, Relaxed);
    let mut data = Vec::new();
    data.frame(|out| out.write(&[3, 0]));
    data.frame(|out| out.write(&[4, 0]));
    let source = AnyValue::from_encoded(&NESTED, data);
    let mut width = AnyWidth::default();
    (NESTED.copy)(source.data(), &mut width);
    assert_eq!(width.0, source.data().len());
    assert_eq!(COPIES.load(Relaxed), 0);
    let retained = source.retain()?;
    assert_eq!(COPIES.load(Relaxed), 2);
    let alias = retained.clone();
    assert_eq!(COPIES.load(Relaxed), 4);
    let mut destination = AnyValue::default();
    destination.reserve(width.0)?;
    let ((), allocations) = hgl_alloc_count::count_in(|| destination.copy_prepared(&source));
    assert_eq!(allocations, 0);
    assert_eq!(COPIES.load(Relaxed), 6);
    destination.copy_prepared(&AnyValue::default());
    drop(source);
    assert!(destination.data().is_empty());
    assert_eq!(
        Frames(retained.data())
            .map(|v| v.to_vec())
            .collect::<Vec<_>>(),
        [&[3, 1], &[4, 1]]
    );
    assert_eq!(
        Frames(alias.data()).map(|v| v.to_vec()).collect::<Vec<_>>(),
        [&[3, 2], &[4, 2]]
    );
    let empty = AnyValue::from_encoded(&PLAN, Vec::new()).retain()?;
    assert!(empty.data().is_empty());
    assert_eq!(COPIES.load(Relaxed), 7);
    Ok(())
}
