//! A replacement may read its own installed owner until the complete encoding is ready.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{Any, PreparedValue, ValueColumns};
use hgl_types::{AnyOperation, AnyPlan, AnyValue, NodeError, NodeResult};
use std::{any::TypeId, cell::Cell};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
static PLAN: AnyPlan = AnyPlan {
    capture: None,
    validate: None,
    copy: |data, out| out.write(&data),
    execution: &hgl_store::any_values::operations::EXECUTION,
    identity: TypeId::of::<i64>,
    operation: AnyOperation::Integer,
    capabilities: [true; 3],
};
#[test]
fn repeated_self_composition_preserves_sources_without_allocating() -> NodeResult {
    let mut columns = ValueColumns::default();
    let target = Any::allocate(&mut columns, &64)?;
    let snapshot = Any::allocate(&mut columns, &64)?;
    let original = AnyValue::from_encoded(&PLAN, 7_i64.to_le_bytes().to_vec());
    Any::copy_native(&mut columns, target, &original);
    Any::copy_within(&mut columns, target, snapshot);
    let (result, allocations) = count_in(|| -> NodeResult {
        for _ in 0..32 {
            columns.compose_any(
                target,
                |columns| {
                    let source = columns.any(target.fields());
                    Ok((source.plan(), source.data().len()))
                },
                |columns, output| (PLAN.copy)(columns.any(target.fields()).data(), output),
            )?;
            assert!(columns.any(target.fields()).equal(&original)?);
        }
        let replacement = AnyValue::default();
        Any::copy_native(&mut columns, target, &replacement);
        assert!(columns.any(snapshot.fields()).equal(&original)?);
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    Ok(())
}
#[test]
fn failed_measurement_or_capacity_never_replaces_the_installed_owner() -> NodeResult {
    let mut columns = ValueColumns::default();
    let target = Any::allocate(&mut columns, &8)?;
    let original = AnyValue::from_encoded(&PLAN, 7_i64.to_le_bytes().to_vec());
    Any::copy_native(&mut columns, target, &original);
    let measured = Cell::new(false);
    let composed = Cell::new(false);
    let failure = columns
        .compose_any(
            target,
            |_| {
                measured.set(true);
                Err(NodeError::coded("measurement failed", "test.measurement"))
            },
            |_, _| composed.set(true),
        )
        .unwrap_err();
    assert_eq!(failure.code, Some("test.measurement"));
    assert!(measured.get());
    assert!(!composed.get());
    assert!(columns.any(target.fields()).equal(&original)?);
    let failure = columns
        .compose_any(
            target,
            |_| Ok((Some(&PLAN), usize::MAX)),
            |_, _| composed.set(true),
        )
        .unwrap_err();
    assert!(failure.message.contains("capacity"));
    assert!(!composed.get());
    assert!(columns.any(target.fields()).equal(&original)?);
    Ok(())
}
