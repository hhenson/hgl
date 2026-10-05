//! Prepared rolling arrivals retain independent values and update only on publication.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::shapes::{Input, Output};
use hgl_store::{Rolling, Store, Wake};
use hgl_types::{EngineTime, NodeId, NodeResult};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
struct Wakes;
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {}
}
fn time(value: i64) -> EngineTime {
    EngineTime::from_micros(value)
}
fn prepare<S: hgl_store::WindowShape<Payload = String>>(
    store: &mut Store,
    owner: u32,
) -> Result<(Output<S>, Input<S>), Box<hgl_types::NodeError>> {
    let id = store.add_shaped_output(NodeId(owner), S::shape());
    let input = store.add_shaped_input(NodeId(owner + 10), S::shape(), true);
    store
        .bind(input, id)
        .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?;
    let storage = store.prepared();
    storage
        .rolling
        .prepare_output::<S>(storage.bindings, id, &8, 8)?;
    Ok((
        Output::bind(store.bindings(), id)
            .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?,
        Input::bind(store.bindings(), input)
            .map_err(|error| hgl_types::NodeError::new(format!("{error:?}")))?,
    ))
}
#[test]
fn inclusive_duration_eviction_idle_readiness_and_delayed_output_are_independent()
-> Result<(), Box<hgl_types::NodeError>> {
    type S = Rolling<String, true, 5, 1>;
    let mut store = Store::new();
    let (output, input) = prepare::<S>(&mut store, 0)?;
    let (later, later_input) = prepare::<S>(&mut store, 1)?;
    let mut wakes = Wakes;
    let first = String::from("first");
    let second = String::from("second");
    let (result, allocations) = count_in(|| -> NodeResult {
        store
            .prepared()
            .tick(time(1), NodeId(0), &mut wakes)
            .rolling(output, &first)?;
        store
            .prepared()
            .tick(time(3), NodeId(1), &mut wakes)
            .pass_rolling(input, later)?;
        store
            .prepared()
            .tick(time(3), NodeId(0), &mut wakes)
            .rolling(output, &second)?;
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(store.rolling.ready(store.bindings(), input));
    assert!(!store.rolling.ready(store.bindings(), later_input));
    assert_eq!(
        store
            .rolling
            .borrow(store.bindings(), later_input)?
            .read(store.rolling.values())?,
        "first"
    );
    let (result, allocations) = count_in(|| {
        store
            .prepared()
            .tick(time(9), NodeId(1), &mut wakes)
            .pass_rolling(input, later)
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(store.rolling.ready(store.bindings(), input));
    assert!(!store.rolling.ready(store.bindings(), later_input));
    assert_eq!(
        store
            .rolling
            .borrow(store.bindings(), later_input)?
            .read(store.rolling.values())?,
        "second"
    );
    let (result, allocations) = count_in(|| {
        store
            .prepared()
            .tick(time(8), NodeId(0), &mut wakes)
            .rolling(output, &first)
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(store.rolling.ready(store.bindings(), input));
    let too_large = "longer than the prepared payload".to_owned();
    assert!(
        store
            .prepared()
            .tick(time(20), NodeId(0), &mut wakes)
            .rolling(output, &too_large)
            .is_err()
    );
    assert!(store.rolling.ready(store.bindings(), input));
    assert_eq!(store.bindings().last_modified(input.id()), time(8));
    let (result, allocations) = count_in(|| {
        store
            .prepared()
            .tick(time(20), NodeId(0), &mut wakes)
            .rolling(output, &second)
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(!store.rolling.ready(store.bindings(), input));
    Ok(())
}
#[test]
fn equal_tick_arrivals_reuse_ring_slots_and_removal_resets_readiness()
-> Result<(), Box<hgl_types::NodeError>> {
    type S = Rolling<String, false, 2, 2>;
    let mut store = Store::new();
    let (output, input) = prepare::<S>(&mut store, 0)?;
    let mut wakes = Wakes;
    let mut value = String::from("same");
    for tick in 1..=10 {
        let (result, allocations) = count_in(|| {
            store
                .prepared()
                .tick(time(tick), NodeId(0), &mut wakes)
                .rolling(output, &value)
        });
        result?;
        assert_eq!(allocations, 0);
        assert_eq!(store.rolling.ready(store.bindings(), input), tick >= 2);
        assert_eq!(store.bindings().last_modified(input.id()), time(tick));
    }
    value.clear();
    assert_eq!(
        store
            .rolling
            .borrow(store.bindings(), input)?
            .read(store.rolling.values())?,
        "same"
    );
    store
        .prepared()
        .bindings
        .invalidate(output.id(), time(11), &mut wakes);
    assert!(!store.rolling.ready(store.bindings(), input));
    let (result, allocations) = count_in(|| {
        store
            .prepared()
            .tick(time(12), NodeId(0), &mut wakes)
            .rolling(output, &value)
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(!store.rolling.ready(store.bindings(), input));
    assert_eq!(
        store
            .rolling
            .borrow(store.bindings(), input)?
            .read(store.rolling.values())?,
        ""
    );
    Ok(())
}
