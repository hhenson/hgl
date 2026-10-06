//! NAT-3/6: borrowed observations preserve the accepted input semantics.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_stdlib::native::{InputView, bit_and_i64};
use hgl_store::{BindError, InputId, Kind, Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn time(n: i64) -> EngineTime {
    EngineTime::from_micros(n)
}
fn observations(store: &Store, input: InputId, n: i64) -> (bool, bool, bool, EngineTime) {
    let view = InputView::new(store, input, time(n));
    (
        view.valid(),
        view.all_valid(),
        view.modified(),
        view.last_modified(),
    )
}

#[test]
fn nat3_bitwise_values_do_not_become_boolean_or_logical_operations() {
    assert_eq!(bit_and_i64(6, 3), 2);
    assert_eq!(bit_and_i64(-1, 7), 7);
    assert_eq!(bit_and_i64(i64::MIN, -1), i64::MIN);
    assert_eq!(bit_and_i64(i64::MAX, i64::MIN), 0);
}

#[test]
fn nat3_unbound_and_idle_queries_do_not_allocate() -> Result<(), BindError> {
    let mut store = Store::new();
    let input = store.add_input::<i64>(NodeId(1), false);
    assert_eq!(
        observations(&store, input.id(), 1),
        (false, false, false, time(0))
    );
    let output = store.add_output::<i64>(NodeId(0));
    store.bind(input.id(), output.id())?;
    assert_eq!(
        observations(&store, input.id(), 1),
        (false, false, false, time(0))
    );
    store.set(output, 0, time(1), NodeId(0), &mut Quiet);
    let (observed, allocations) = count_in(|| observations(&store, input.id(), 1));
    assert_eq!(allocations, 0);
    assert_eq!(observed, (true, true, true, time(1)));
    store.begin_cycle(time(2));
    assert_eq!(
        observations(&store, input.id(), 2),
        (true, true, false, time(1))
    );
    Ok(())
}

#[test]
fn nat6_fixed_views_observe_peered_and_assembled_invalidation() -> Result<(), BindError> {
    for bundle in [false, true] {
        for assembled in [false, true] {
            let scalar = Kind::Ts(ScalarType::I64);
            let shape = if bundle {
                Kind::Bundle(vec![
                    ("left".into(), scalar.clone()),
                    ("right".into(), scalar),
                ])
            } else {
                Kind::List(Box::new(scalar), 2)
            };
            let mut store = Store::new();
            let output = store.add_shaped_output(NodeId(0), shape.clone());
            let input = store.add_shaped_input(NodeId(1), shape.clone(), true);
            let left = store.bindings().fixed_output(output, 0);
            let right = store.bindings().fixed_output(output, 1);
            if assembled {
                let children = vec![store.reference(left), store.reference(right)];
                let designation = store.items_reference(shape, children)?;
                store.sample(input, designation, time(0), &mut Quiet)?;
            } else {
                store.bind(input, output)?;
            }
            let lhs = store.scalar_output::<i64>(left)?;
            let rhs = store.scalar_output::<i64>(right)?;
            store.set(lhs, 1, time(1), NodeId(0), &mut Quiet);
            assert_eq!(observations(&store, input, 1), (true, false, true, time(1)));
            store.set(rhs, 2, time(2), NodeId(0), &mut Quiet);
            assert_eq!(observations(&store, input, 2), (true, true, true, time(2)));
            store.invalidate(left, time(3), &mut Quiet);
            assert_eq!(observations(&store, input, 3), (true, false, true, time(3)));
            if assembled {
                let child = store.bindings().fixed_input(input, 0);
                assert_eq!(
                    observations(&store, child, 3),
                    (false, false, true, time(3))
                );
            }
            store.invalidate(if assembled { right } else { output }, time(4), &mut Quiet);
            assert_eq!(
                observations(&store, input, 4),
                (false, false, false, time(0))
            );
        }
    }
    Ok(())
}

#[test]
fn nat6_nested_all_valid_means_immediate_children() -> Result<(), BindError> {
    let leaf = Kind::Ts(ScalarType::I64);
    let pair = Kind::List(Box::new(leaf), 2);
    let shape = Kind::Bundle(vec![("left".into(), pair.clone()), ("right".into(), pair)]);
    let mut store = Store::new();
    let output = store.add_shaped_output(NodeId(0), shape.clone());
    let input = store.add_shaped_input(NodeId(1), shape, true);
    store.bind(input, output)?;
    for n in 0..2 {
        let child = store.bindings().fixed_output(output, n);
        let leaf = store.bindings().fixed_output(child, 0);
        store.set(
            store.scalar_output::<i64>(leaf)?,
            1,
            time(1),
            NodeId(0),
            &mut Quiet,
        );
    }
    assert!(InputView::new(&store, input, time(1)).all_valid());
    let child = store.bindings().fixed_input(input, 0);
    assert!(!InputView::new(&store, child, time(1)).all_valid());
    Ok(())
}

#[test]
fn nat6_reference_rebind_and_scope_retirement() -> Result<(), BindError> {
    let mut store = Store::new();
    let input = store.add_input::<i64>(NodeId(1), false);
    let scope = store.child_scope(NodeId(0));
    let parent = store.enter_scope(scope);
    let output = store.add_output::<i64>(NodeId(0));
    store.set(output, 42, time(1), NodeId(0), &mut Quiet);
    let target = store.reference(output.id());
    store.enter_scope(parent);
    store.sample(input.id(), target, time(1), &mut Quiet)?;
    assert_eq!(
        observations(&store, input.id(), 1),
        (true, true, true, time(1))
    );
    store.release_scope(scope, time(2), &mut Quiet);
    assert_eq!(
        observations(&store, input.id(), 2),
        (true, true, false, time(1))
    );
    store.begin_cycle(time(3));
    assert_eq!(
        observations(&store, input.id(), 3),
        (false, false, false, time(0))
    );
    store.sample(input.id(), Reference::default(), time(3), &mut Quiet)?;
    assert_eq!(
        observations(&store, input.id(), 3),
        (false, false, false, time(0))
    );
    Ok(())
}

#[test]
fn nat6_dictionary_removal_retains_member_until_next_cycle() -> Result<(), BindError> {
    let mut store = Store::new();
    let output = store.add_dictionary::<i64>(NodeId(0));
    let input = store.add_dictionary_input::<i64>(NodeId(1), true);
    store.bind(input.id(), output.id())?;
    let child = store.get_or_create(output, 7, time(1), &mut Quiet);
    store.set(child, 42, time(1), NodeId(0), &mut Quiet);
    let member = store.child(input, 7).ok_or(BindError::InvalidReference)?;
    assert!(InputView::new(&store, member.id(), time(1)).valid());
    store.remove(output, 7, time(2), &mut Quiet);
    assert!(store.child(input, 7).is_none());
    assert_eq!(
        observations(&store, input.id(), 2),
        (true, true, true, time(2))
    );
    assert!(InputView::new(&store, member.id(), time(2)).valid());
    store.begin_cycle(time(3));
    assert_eq!(
        observations(&store, member.id(), 3),
        (false, false, false, time(0))
    );
    Ok(())
}
