//! Finite ordinary copies own independent backing storage before the first cycle.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::shapes::{Atomic, Input, Output, Shape};
use hgl_store::{
    List, ListBounds, PreparedValue, Store, ValueColumns, Wake, append_slot, commit_append,
};
use hgl_types::{EngineTime, NodeError, NodeId, NodeResult, Time, ZoneId, ZonedTime};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[derive(Default)]
struct Wakes(usize);
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {
        self.0 += 1;
    }
}
type Nested = List<List<String>>;
#[test]
fn first_repeated_empty_and_larger_copies_reuse_independent_descendants() -> NodeResult {
    let first = vec![
        vec!["first".into()],
        vec!["longer identity".into(), String::new()],
    ];
    let second = vec![vec!["replacement".into(), "other".into()]];
    let empty = Vec::new();
    let mut bounds = ListBounds::<List<String>>::default();
    Nested::include(&mut bounds, &first);
    Nested::include(&mut bounds, &second);
    let mut columns = ValueColumns::default();
    let source = Nested::allocate(&mut columns, &bounds)?;
    let saved = Nested::allocate(&mut columns, &bounds)?;
    let mut records = ValueColumns::default();
    let record = Nested::allocate(&mut records, &bounds)?;
    let counts = columns.slot_counts();
    let (result, allocations) = count_in(|| -> NodeResult {
        Nested::check_native(&columns, source, &first)?;
        Nested::copy_native(&mut columns, source, &first);
        Nested::check_slots(&columns, source, &columns, saved)?;
        Nested::copy_within(&mut columns, source, saved);
        Nested::check_slots(&columns, source, &records, record)?;
        Nested::copy_between(&columns, source, &mut records, record);
        for _ in 0..30 {
            for value in [&empty, &second, &first] {
                Nested::check_native(&columns, source, value)?;
                Nested::copy_native(&mut columns, source, value);
            }
        }
        Nested::check_native(&columns, source, &second)?;
        Nested::copy_native(&mut columns, source, &second);
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(columns.slot_counts(), counts);
    assert_eq!(source.read(&columns)?, second);
    assert_eq!(saved.read(&columns)?, first);
    assert_eq!(record.read(&records)?, first);
    let bad = vec![vec!["ok".into()], vec!["x".repeat(1000)]];
    assert!(Nested::check_native(&columns, source, &bad).is_err());
    assert_eq!(source.read(&columns)?, second);
    Ok(())
}
#[test]
fn atomic_replay_pass_and_global_append_are_allocation_free_and_capture_independently() -> NodeResult
{
    let values = [
        vec!["initial".to_string()],
        Vec::new(),
        vec!["long replacement".into(), "tail".into()],
    ];
    let mut bounds = ListBounds::<String>::default();
    for value in &values {
        List::<String>::include(&mut bounds, value);
    }
    let record_bounds = ListBounds::<List<String>> {
        len: 3,
        element: bounds,
    };
    let mut store = Store::new();
    store.global_state().provision();
    store
        .global_state()
        .prepare_value::<List<List<String>>>("records", &record_bounds)?;
    let record = store.global_state().bind::<List<List<String>>>("records")?;
    let destination = store.global_state().destination(record);
    store.global_state().mark_present(record);
    let a = store.add_atomic_output::<List<String>>(NodeId(0));
    let b = store.add_atomic_output::<List<String>>(NodeId(1));
    for output in [a, b] {
        let prepared = store.prepared();
        prepared.atomic.prepare_output::<List<String>>(
            prepared.bindings,
            output,
            &record_bounds.element,
        )?;
    }
    let input = bind_input(&mut store, a, NodeId(1))?;
    let observed = bind_input(&mut store, b, NodeId(2))?;
    let a = Output::<Atomic<List<String>>>::bind(store.bindings(), a)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let b = Output::<Atomic<List<String>>>::bind(store.bindings(), b)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    assert!(!store.input_valid(input.id()));
    assert!(!store.input_valid(observed.id()));
    let mut wakes = Wakes::default();
    let (result, allocations) = count_in(|| -> NodeResult {
        for (index, value) in values.iter().enumerate() {
            let now = EngineTime::from_micros(
                i64::try_from(index).map_err(|e| NodeError::new(e.to_string()))? + 1,
            );
            store.begin_cycle(now);
            store
                .prepared()
                .tick(now, NodeId(0), &mut wakes)
                .atomic(a, value)?;
            store
                .prepared()
                .tick(now, NodeId(1), &mut wakes)
                .pass_atomic(input, b)?;
            let mut prepared = store.prepared();
            let (observations, globals) = prepared.observations();
            let from = observations
                .atomic
                .borrow(observations.bindings, observed)?;
            let to = append_slot(globals.values(), destination)?;
            List::<String>::check_slots(observations.atomic.values(), from, globals.values(), to)?;
            List::<String>::copy_between(
                observations.atomic.values(),
                from,
                globals.values_mut(),
                to,
            );
            commit_append(globals.values_mut(), destination);
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(store.global_state().get(record)?, values);
    assert!(append_slot(store.global_state().values(), destination).is_err());
    assert_eq!(store.global_state().get(record)?, values);
    Ok(())
}
#[test]
fn exact_zone_names_reuse_capacity_without_aliasing_retained_captures() -> NodeResult {
    let first =
        ZonedTime::from_validated_parts(Time(17), ZoneId::from_validated_name("US/Eastern".into()));
    let second = ZonedTime::from_validated_parts(
        Time(23),
        ZoneId::from_validated_name("America/New_York".into()),
    );
    let mut bounds = 0;
    ZonedTime::include(&mut bounds, &first);
    ZonedTime::include(&mut bounds, &second);
    let mut columns = ValueColumns::default();
    let source = ZonedTime::allocate(&mut columns, &bounds)?;
    let saved = ZonedTime::allocate(&mut columns, &bounds)?;
    let (result, allocations) = count_in(|| -> NodeResult {
        ZonedTime::check_native(&columns, source, &first)?;
        ZonedTime::copy_native(&mut columns, source, &first);
        ZonedTime::check_slots(&columns, source, &columns, saved)?;
        ZonedTime::copy_within(&mut columns, source, saved);
        for _ in 0..100 {
            ZonedTime::check_native(&columns, source, &second)?;
            ZonedTime::copy_native(&mut columns, source, &second);
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(source.read(&columns)?, second);
    assert_eq!(saved.read(&columns)?, first);
    Ok(())
}

fn bind_input(
    store: &mut Store,
    output: hgl_store::OutputId,
    owner: NodeId,
) -> NodeResult<Input<Atomic<List<String>>>> {
    let input = store.add_shaped_input(owner, Atomic::<List<String>>::shape(), true);
    store
        .bind(input, output)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    Input::bind(store.bindings(), input).map_err(|e| NodeError::new(format!("{e:?}")))
}
#[test]
fn failed_prepared_publication_preserves_payload_time_and_unpublished_record_length() -> NodeResult
{
    let mut store = Store::new();
    let id = store.add_atomic_output::<List<String>>(NodeId(0));
    let bounds = ListBounds::<String> { len: 2, element: 4 };
    let prepared = store.prepared();
    prepared
        .atomic
        .prepare_output::<List<String>>(prepared.bindings, id, &bounds)?;
    let input = bind_input(&mut store, id, NodeId(1))?;
    let output = Output::<Atomic<List<String>>>::bind(store.bindings(), id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let mut wake = Wakes::default();
    let value = vec!["old".to_string()];
    store
        .prepared()
        .tick(EngineTime::from_micros(1), NodeId(0), &mut wake)
        .atomic(output, &value)?;
    let bad = vec!["new".to_string(), "oversized".to_string()];
    assert!(
        store
            .prepared()
            .tick(EngineTime::from_micros(2), NodeId(0), &mut wake)
            .atomic(output, &bad)
            .is_err()
    );
    assert_eq!(store.atomic_get(input)?, value);
    assert!(store.output_modified(id, EngineTime::from_micros(1)));
    assert_eq!(wake.0, 1);
    let mut columns = ValueColumns::default();
    let fixed = List::<String, 2>::allocate(&mut columns, &bounds)?;
    assert!(List::<String, 2>::check_native(&columns, fixed, &value).is_err());
    let records = List::<List<String>>::allocate(
        &mut columns,
        &ListBounds {
            len: 1,
            element: bounds,
        },
    )?;
    let vacant = append_slot(&columns, records)?;
    assert!(List::<String>::check_native(&columns, vacant, &bad).is_err());
    assert!(records.read(&columns)?.is_empty());
    Ok(())
}
