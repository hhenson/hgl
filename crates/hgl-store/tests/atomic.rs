//! Whole ordinary snapshots retain independently through temporal endpoint lifetimes.
use hgl_alloc_count::{CountingAllocator, count_in};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
use hgl_store::{
    List, Store, Wake,
    shapes::{Atomic, Fixed, Input, Map, Output, Shape},
};
use hgl_types::{EngineTime, NodeId, NodeResult};

#[derive(Default)]
struct Wakes(Vec<NodeId>);
impl Wake for Wakes {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}
fn time(n: i64) -> EngineTime {
    EngineTime::from_micros(n)
}

#[test]
fn whole_empty_and_equal_snapshots_publish_and_retain_independently() -> NodeResult {
    let mut store = Store::new();
    let id = store.add_atomic_output::<List<List<String>>>(NodeId(0));
    let output = Output::<Atomic<List<List<String>>>>::bind(store.bindings(), id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<List<List<String>>>::shape(), true);
    let input = Input::bind(store.bindings(), input_id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    store
        .bind(input_id, id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    assert!(store.atomic_get::<List<List<String>>>(input).is_err());
    let mut wakes = Wakes::default();
    let source = vec![vec!["original".to_owned()]];
    store.set_atomic(output, source.clone(), time(1), &mut wakes)?;
    let retained = store.atomic_get(input)?;
    store.set_atomic(output, vec![], time(2), &mut wakes)?;
    assert!(store.input_valid(input_id));
    assert!(store.atomic_get(input)?.is_empty());
    store.set_atomic(output, vec![], time(3), &mut wakes)?;
    assert!(store.bindings().modified(input_id, time(3)));
    assert_eq!(wakes.0, [NodeId(1), NodeId(1), NodeId(1)]);
    drop(store);
    assert_eq!(retained, source);
    Ok(())
}

#[test]
fn failed_preparation_neither_changes_value_nor_ticks() -> NodeResult {
    let mut store = Store::new();
    let id = store.add_atomic_output::<List<i64, 2>>(NodeId(0));
    let output = Output::<Atomic<List<i64, 2>>>::bind(store.bindings(), id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<List<i64, 2>>::shape(), true);
    let input = Input::bind(store.bindings(), input_id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    store
        .bind(input_id, id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let mut wakes = Wakes::default();
    assert!(
        store
            .set_atomic(output, vec![9], time(1), &mut wakes)
            .is_err()
    );
    assert!(!store.input_valid(input_id));
    assert!(wakes.0.is_empty());
    store.set_atomic(output, vec![1, 2], time(2), &mut wakes)?;
    assert!(
        store
            .set_atomic(output, vec![], time(3), &mut wakes)
            .is_err()
    );
    assert_eq!(store.atomic_get::<List<i64, 2>>(input)?, [1, 2]);
    assert!(!store.bindings().modified(input_id, time(3)));
    assert_eq!(wakes.0, [NodeId(1)]);
    Ok(())
}

#[test]
fn prepared_borrow_does_not_copy_and_replacement_reuses_descendants() -> NodeResult {
    let mut store = Store::new();
    let id = store.add_atomic_output::<List<String>>(NodeId(0));
    let output = Output::<Atomic<List<String>>>::bind(store.bindings(), id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<List<String>>::shape(), false);
    let input = Input::bind(store.bindings(), input_id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    store
        .bind(input_id, id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let mut wakes = Wakes::default();
    for n in 1..=2 {
        store.set_atomic(output, vec!["payload".repeat(100)], time(n), &mut wakes)?;
    }
    let counts = store.atomic_values().slot_counts();
    let (result, allocations) = count_in(|| -> NodeResult {
        for _ in 0..10_000 {
            let slot = store.atomic_borrow::<List<String>>(input)?;
            let layout = &store.atomic_values().list(slot.fields())[0];
            assert_eq!(store.atomic_values().scalar::<String>(layout[0]).len(), 700);
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    for n in 3..=100 {
        store.set_atomic(output, vec!["new".repeat(100)], time(n), &mut wakes)?;
    }
    assert_eq!(store.atomic_values().slot_counts(), counts);
    assert!(wakes.0.is_empty());
    Ok(())
}

#[test]
fn atomic_children_publish_through_fixed_and_keyed_parents() -> NodeResult {
    type Item = Atomic<List<i64>>;
    let mut store = Store::new();
    let fixed_id = store.add_shaped_output(NodeId(0), Fixed::<Item, 1>::shape());
    let fixed = Output::<Fixed<Item, 1>>::bind(store.bindings(), fixed_id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let fixed_input = store.add_shaped_input(NodeId(1), Fixed::<Item, 1>::shape(), true);
    store
        .bind(fixed_input, fixed_id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let mut wakes = Wakes::default();
    store.set_atomic(
        fixed.index(store.bindings(), 0),
        vec![],
        time(1),
        &mut wakes,
    )?;
    assert!(store.input_valid(fixed_input));
    let map_id = store.add_shaped_output(NodeId(0), Map::<Item>::shape());
    let map_input = store.add_shaped_input(NodeId(2), Map::<Item>::shape(), true);
    store
        .bind(map_input, map_id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let child = store.get_or_create_with(
        map_id,
        7,
        time(2),
        &mut wakes,
        Store::add_atomic_output::<List<i64>>,
    );
    let output = Output::<Atomic<List<i64>>>::bind(store.bindings(), child)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    store.set_atomic(output, vec![], time(2), &mut wakes)?;
    let input = Input::<Map<Item>>::bind(store.bindings(), map_input)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?
        .member(store.bindings(), 7)
        .ok_or_else(|| hgl_types::NodeError::new("missing child"))?;
    assert!(store.atomic_get(input)?.is_empty());
    assert!(store.bindings().modified(map_input, time(2)));
    Ok(())
}

#[test]
fn scope_reuse_does_not_revive_stale_atomic_writing_handles() -> NodeResult {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let scope = store.child_scope(NodeId(0));
    let parent = store.enter_scope(scope);
    let id = store.add_atomic_output::<List<i64>>(NodeId(0));
    let expired = Output::<Atomic<List<i64>>>::bind(store.bindings(), id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    store.set_atomic(expired, vec![1], time(1), &mut wakes)?;
    store.enter_scope(parent);
    store.release_scope(scope, time(2), &mut wakes);
    store.begin_cycle(time(3));
    let next = store.child_scope(NodeId(0));
    store.enter_scope(next);
    let reused = store.add_atomic_output::<List<i64>>(NodeId(0));
    assert_eq!(reused, id);
    let output = Output::<Atomic<List<i64>>>::bind(store.bindings(), reused)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    assert_ne!(expired.generation(), output.generation());
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.set_atomic(expired, vec![2], time(3), &mut wakes)
    }));
    assert!(rejected.is_err());
    assert!(!store.output_modified(reused, time(3)));
    store.set_atomic(output, vec![3], time(3), &mut wakes)?;
    Ok(())
}

struct Silent;
impl Wake for Silent {
    fn wake(&mut self, _: NodeId) {}
}

#[test]
fn warmed_empty_publications_do_not_allocate_bookkeeping() -> NodeResult {
    let mut store = Store::new();
    let id = store.add_atomic_output::<List<i64>>(NodeId(0));
    let output = Output::<Atomic<List<i64>>>::bind(store.bindings(), id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    store.set_atomic(output, Vec::new(), time(1), &mut Silent)?;
    let (result, allocations) = count_in(|| -> NodeResult {
        for now in 2..=101 {
            store.set_atomic(output, Vec::new(), time(now), &mut Silent)?;
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(store.output_modified(id, time(101)));
    Ok(())
}

#[test]
fn preparation_scratch_resets_after_partial_failure_and_success() -> NodeResult {
    type Payload = List<List<i64, 2>>;
    let mut store = Store::new();
    let id = store.add_atomic_output::<Payload>(NodeId(0));
    let output = Output::<Atomic<Payload>>::bind(store.bindings(), id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<Payload>::shape(), false);
    let input = Input::bind(store.bindings(), input_id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    store
        .bind(input_id, id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    for cycle in 0..20 {
        let failed_time = time(cycle * 3 + 1);
        assert!(
            store
                .set_atomic(output, vec![vec![1, 2], vec![3]], failed_time, &mut Silent)
                .is_err()
        );
        assert!(!store.output_modified(id, failed_time));
        if cycle == 0 {
            assert!(!store.input_valid(input_id));
        } else {
            assert!(store.atomic_get::<Payload>(input)?.is_empty());
        }
        let value = vec![vec![cycle, cycle + 1]];
        store.set_atomic(output, value.clone(), time(cycle * 3 + 2), &mut Silent)?;
        assert_eq!(store.atomic_get::<Payload>(input)?, value);
        store.set_atomic(output, Vec::new(), time(cycle * 3 + 3), &mut Silent)?;
        assert!(store.atomic_get::<Payload>(input)?.is_empty());
    }
    Ok(())
}
