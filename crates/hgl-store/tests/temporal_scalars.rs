//! New scalar leaves retain exact identities through typed storage boundaries.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{
    List, Scalar, Store, Wake,
    shapes::{Atomic, Input, Output, Shape},
};
use hgl_types::{CivilDateTime, EngineTime, NodeError, NodeId, NodeResult, ZoneId, ZonedDateTime};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[derive(Default)]
struct Wakes(usize);
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {
        self.0 += 1;
    }
}
fn at(value: i64) -> EngineTime {
    EngineTime::from_micros(value)
}
fn exercise<T: Scalar + Shape>(first: T, second: T) -> NodeResult {
    let mut store = Store::new();
    let output = store.add_output::<T>(NodeId(0));
    let input = store.add_input::<T>(NodeId(1), true);
    store
        .bind(input.id(), output.id())
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    let mut wakes = Wakes::default();
    store.set(output, first.try_clone()?, at(1), NodeId(0), &mut wakes);
    store.set(output, first.try_clone()?, at(2), NodeId(0), &mut wakes);
    assert_eq!(wakes.0, 2);
    assert!(store.modified(input, at(2)));
    let ((), allocations) = count_in(|| {
        for _ in 0..10_000 {
            assert_eq!(store.get_ref(input), &first);
        }
    });
    assert_eq!(allocations, 0);
    let retained = store.get_ref(input).try_clone()?;
    store.set(output, second.try_clone()?, at(3), NodeId(0), &mut wakes);
    store.global_state().provision();
    let global = store.global_state().bind::<List<T>>("temporal")?;
    store
        .global_state()
        .set(global, &vec![first.try_clone()?])?;
    let saved = store.global_state().get(global)?;
    let (borrowed, allocations) = count_in(|| {
        for _ in 0..10_000 {
            let _slot = store.global_state().borrow(global)?;
        }
        Ok::<_, Box<NodeError>>(())
    });
    borrowed?;
    assert_eq!(allocations, 0);
    store
        .global_state()
        .set(global, &vec![second.try_clone()?])?;
    let atomic_id = store.add_atomic_output::<List<T>>(NodeId(2));
    let atomic = Output::<Atomic<List<T>>>::bind(store.bindings(), atomic_id)
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    let id = store.add_shaped_input(NodeId(3), Atomic::<List<T>>::shape(), false);
    let observer = Input::<Atomic<List<T>>>::bind(store.bindings(), id)
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    store
        .bind(id, atomic_id)
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    store.set_atomic(atomic, vec![first.try_clone()?], at(4), &mut wakes)?;
    let captured = store.atomic_get(observer)?;
    store.set_atomic(atomic, vec![second], at(5), &mut wakes)?;
    drop(store);
    assert_eq!(retained, first);
    assert_eq!(saved, vec![first.try_clone()?]);
    assert_eq!(captured, vec![first]);
    Ok(())
}
#[test]
fn typed_civil_zone_and_zoned_values_survive_replacement_and_teardown() -> NodeResult {
    exercise(
        CivilDateTime::from_micros(123_456),
        CivilDateTime::from_micros(-1),
    )?;
    exercise(
        ZoneId::from_validated_name("US/Eastern".into()),
        ZoneId::from_validated_name("America/New_York".into()),
    )?;
    exercise(
        ZonedDateTime::from_validated_parts(at(0), ZoneId::from_validated_name("UTC".into()), 0),
        ZonedDateTime::from_validated_parts(
            at(0),
            ZoneId::from_validated_name("US/Eastern".into()),
            -18_000,
        ),
    )
}
