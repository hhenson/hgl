//! Nominal enum roots prepare storage before any publication, preserving exact types.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{
    Capacity, GlobalValue, Layouts, List, Store, ValueColumns, Wake,
    shapes::{Atomic, Input, Output, Shape},
};
use hgl_types::{EngineTime, NodeError, NodeId, NodeResult, OrdinaryType};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[derive(Debug)]
struct Enum<const ID: bool>;
impl<const ID: bool> GlobalValue for Enum<ID> {
    const PREPARED_SCALAR: bool = true;
    const WIDTH: usize = 1;
    type Value = i64;
    type Slots = usize;
    fn schema() -> OrdinaryType {
        OrdinaryType::Enum(if ID { "sample::Other" } else { "sample::Mode" })
    }
    fn slots(layout: &mut &[usize]) -> usize {
        <i64 as GlobalValue>::slots(layout)
    }
    fn flatten(slot: usize, layout: &mut [usize]) {
        <i64 as GlobalValue>::flatten(slot, layout);
    }
    fn retain(value: &i64) -> NodeResult<i64> {
        Ok(*value)
    }
    fn prepare(value: &i64, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        <i64 as GlobalValue>::prepare(value, capacity, layouts)
    }
    fn read(columns: &ValueColumns, slot: usize) -> NodeResult<i64> {
        <i64 as GlobalValue>::read(columns, slot)
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: i64, layouts: &mut Layouts) {
        <i64 as GlobalValue>::commit(columns, slot, value, layouts);
    }
    fn install(columns: &mut ValueColumns, value: i64, layouts: &mut Layouts) -> usize {
        <i64 as GlobalValue>::install(columns, value, layouts)
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        <i64 as GlobalValue>::release(columns, slot);
    }
}
#[derive(Default)]
struct Wakes(usize);
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {
        self.0 += 1;
    }
}
#[test]
fn first_and_repeated_enum_publications_allocate_nothing() -> NodeResult {
    let mut store = Store::new();
    let mut outputs = Vec::new();
    for _ in 0..8 {
        let id = store.add_atomic_output::<Enum<false>>(NodeId(0));
        outputs.push(
            Output::<Atomic<Enum<false>>>::bind(store.bindings(), id)
                .map_err(|error| NodeError::new(format!("{error:?}")))?,
        );
    }
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<Enum<false>>::shape(), true);
    store
        .bind(input_id, outputs[7].id())
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    let input = Input::<Atomic<Enum<false>>>::bind(store.bindings(), input_id)
        .map_err(|error| NodeError::new(format!("{error:?}")))?;
    assert!(!store.input_valid(input_id));
    let other_id = store.add_shaped_input(NodeId(2), Atomic::<Enum<true>>::shape(), true);
    assert!(store.bind(other_id, outputs[7].id()).is_err());
    let integer = store.add_input::<i64>(NodeId(2), false);
    assert!(store.bind(integer.id(), outputs[7].id()).is_err());
    let mut wakes = Wakes::default();
    let (result, allocations) = count_in(|| -> NodeResult {
        for tick in 1..=100 {
            let value = if tick % 2 == 0 { i64::MAX } else { i64::MIN };
            for output in &outputs {
                store.set_atomic(*output, value, EngineTime::from_micros(tick), &mut wakes)?;
            }
            assert_eq!(store.atomic_get(input)?, value);
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(wakes.0, 100);
    Ok(())
}
#[test]
fn enum_lists_and_globals_preserve_nominal_identity_and_independent_capture() -> NodeResult {
    let mut store = Store::new();
    store.global_state().provision();
    let global = store.global_state().bind::<List<Enum<false>>>("members")?;
    assert!(
        store
            .global_state()
            .bind::<List<Enum<true>>>("members")
            .is_err()
    );
    store
        .global_state()
        .set(global, &vec![i64::MIN, -7, i64::MAX])?;
    let saved = store.global_state().get(global)?;
    store.global_state().set(global, &vec![11])?;
    drop(store);
    assert_eq!(saved, vec![i64::MIN, -7, i64::MAX]);
    Ok(())
}
