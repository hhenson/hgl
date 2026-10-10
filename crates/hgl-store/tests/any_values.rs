//! Precompiled arbitrary payload plans retain independent prepared owners and keys.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{Any, Key, Keys, PreparedValue, ValueColumns};
use hgl_types::{AnyOperation, AnyPlan, AnyValue, NodeResult};
use std::{any::TypeId, cmp::Ordering};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
fn identity<T: 'static>() -> TypeId {
    TypeId::of::<T>()
}
static INTEGER: AnyPlan = AnyPlan {
    index: 0,
    identity: identity::<i64>,
    operation: AnyOperation::Integer,
    capabilities: [true; 3],
};
static LIST: AnyPlan = AnyPlan {
    index: 1,
    identity: identity::<Vec<i64>>,
    operation: AnyOperation::Sequence(&INTEGER, false),
    capabilities: [true; 3],
};
static DELTA: AnyPlan = AnyPlan {
    index: 2,
    identity: identity::<(Vec<(i64, i64)>,)>,
    operation: AnyOperation::Data,
    capabilities: [false; 3],
};
static NO_ORDER: AnyPlan = AnyPlan {
    index: 3,
    identity: identity::<Vec<String>>,
    operation: AnyOperation::Sequence(&INTEGER, false),
    capabilities: [true, true, false],
};
static NO_EQUAL: AnyPlan = AnyPlan {
    index: 4,
    identity: identity::<Vec<(Vec<(i64, i64)>,)>>,
    operation: AnyOperation::Sequence(&DELTA, false),
    capabilities: [false; 3],
};
fn list(values: &[i64]) -> AnyValue {
    let mut bytes = Vec::new();
    for value in values {
        bytes.extend_from_slice(&8_u64.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    AnyValue::from_encoded(&LIST, bytes)
}
#[test]
fn aggregate_and_delta_boxes_keep_independent_prepared_contents() -> NodeResult {
    let mut columns = ValueColumns::default();
    let original = Any::allocate(&mut columns, &64)?;
    let saved = Any::allocate(&mut columns, &64)?;
    let aggregate = list(&[1, 2]);
    let replacement = list(&[9]);
    let delta = AnyValue::from_encoded(&DELTA, vec![0, 1, 0, 0, 255]);
    let delta_saved = Any::allocate(&mut columns, &64)?;
    let (result, allocations) = count_in(|| -> NodeResult {
        Any::check_native(&columns, original, &aggregate)?;
        Any::copy_native(&mut columns, original, &aggregate);
        Any::check_slots(&columns, original, &columns, saved)?;
        Any::copy_within(&mut columns, original, saved);
        Any::copy_native(&mut columns, original, &replacement);
        Any::check_native(&columns, delta_saved, &delta)?;
        Any::copy_native(&mut columns, delta_saved, &delta);
        assert!(columns.any(saved.fields()).equal(&aggregate)?);
        assert_eq!(
            columns.any(original.fields()).compare(&aggregate)?,
            Some(Ordering::Greater)
        );
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(delta_saved.read(&columns)?.data(), delta.data());
    assert_eq!(
        (delta_saved.read(&columns)?.plan().unwrap().identity)(),
        identity::<(Vec<(i64, i64)>,)>()
    );
    assert_eq!(
        columns
            .any(delta_saved.fields())
            .equal(&delta)
            .unwrap_err()
            .code,
        Some("value.capability")
    );
    let too_large = list(&[0; 8]);
    assert!(Any::check_native(&columns, original, &too_large).is_err());
    assert!(columns.any(original.fields()).equal(&replacement)?);
    Ok(())
}
#[test]
fn actual_contained_capabilities_are_required_even_for_empty_collections() -> NodeResult {
    let empty_list = AnyValue::from_encoded(&NO_ORDER, Vec::new());
    assert_eq!(
        empty_list.compare(&empty_list).unwrap_err().code,
        Some("value.capability")
    );
    let empty_delta_list = AnyValue::from_encoded(&NO_EQUAL, Vec::new());
    assert_eq!(
        empty_delta_list.equal(&empty_delta_list).unwrap_err().code,
        Some("value.capability")
    );
    assert_eq!(
        empty_delta_list.key_hash().unwrap_err().code,
        Some("value.capability")
    );
    assert!(!empty_delta_list.equal(&empty_list)?);
    assert_eq!(
        AnyValue::default().compare(&empty_delta_list)?,
        Some(Ordering::Less)
    );
    let mut keys = Keys::default();
    let first = list(&[1, 2]);
    <Any as Key>::prepare(&mut keys, &first)?;
    <Any as Key>::prepare(&mut keys, &first.retain()?)?;
    <Any as Key>::prepare(&mut keys, &AnyValue::default())?;
    assert_eq!(<Any as Key>::ids(&keys).len(), 2);
    let (result, allocations) = count_in(|| -> NodeResult {
        for _ in 0..100 {
            assert_eq!(<Any as Key>::id(&keys, &first)?, 0);
            <Any as Key>::with_value(&keys, 0, |v| v.equal(&first))??;
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(
        <Any as Key>::prepare(&mut keys, &empty_delta_list)
            .unwrap_err()
            .code,
        Some("value.capability")
    );
    Ok(())
}
