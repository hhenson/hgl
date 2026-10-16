//! Internal passive owner views preserve typed child endpoints and consumer ordering.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{BindError, Kind, Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn scalar() -> Kind {
    Kind::Ts(ScalarType::I64)
}
#[test]
fn owned_views_reject_active_unowned_mismatched_and_started_preparation() {
    let mut store = Store::new();
    let own = store.add_output::<i64>(NodeId(1));
    let active = store.add_input::<i64>(NodeId(1), true);
    assert_eq!(
        store.prepare_owned_view(active.id(), own.id()),
        Err(BindError::BackwardReference)
    );
    let unowned = store.add_input::<i64>(NodeId(2), false);
    assert_eq!(
        store.prepare_owned_view(unowned.id(), own.id()),
        Err(BindError::BackwardReference)
    );
    let wrong = store.add_input::<bool>(NodeId(1), false);
    assert_eq!(
        store.prepare_owned_view(wrong.id(), own.id()),
        Err(BindError::ShapeMismatch)
    );
    let consumer = store.add_input::<i64>(NodeId(1), false);
    assert_eq!(
        store.sample(
            consumer.id(),
            store.reference(own.id()),
            EngineTime::NEVER,
            &mut Quiet
        ),
        Err(BindError::BackwardReference)
    );
    let view = store.add_input::<i64>(NodeId(1), false);
    store.reserve_scope(store.scope(), 3);
    assert_eq!(
        store.prepare_owned_view(view.id(), own.id()),
        Err(BindError::BackwardReference)
    );
}
#[test]
fn descendant_carrier_retarget_updates_passive_view_without_value_copy_or_allocation()
-> Result<(), BindError> {
    let mut store = Store::new();
    let parent = store.scope();
    let shape = Kind::Bundle(vec![("value".into(), scalar()), ("error".into(), scalar())]);
    let bundle = store.add_shaped_input(NodeId(1), shape, false);
    let error = store.add_output::<i64>(NodeId(1));
    let child = store.child_scope(NodeId(1));
    store.enter_scope(child);
    let first = store.add_output::<i64>(NodeId(0));
    let second = store.add_output::<i64>(NodeId(1));
    let carrier = store.add_reference(NodeId(2), ScalarType::I64, false);
    store.enter_scope(parent);
    let value = store.bindings().fixed_input(bundle, 0);
    let failure = store.bindings().fixed_input(bundle, 1);
    store.prepare_owned_view(value, carrier)?;
    store.prepare_owned_view(failure, error.id())?;
    let initial = store.assemble_owned_view(bundle)?;
    assert_eq!(store.bindings().input_reference(bundle), initial);
    let view = store.scalar_input::<i64>(value)?;
    store.reserve_reference_target(first.id(), 1);
    store.reserve_reference_target(second.id(), 1);
    let start = EngineTime::MIN_START;
    store.enter_scope(child);
    store.set(first, 7, start, NodeId(0), &mut Quiet);
    store.set(second, 9, start, NodeId(1), &mut Quiet);
    for (output, expected) in [(first, 7), (second, 9), (second, 9)] {
        let target = store.reference(output.id());
        let (result, allocations) =
            count_in(|| store.set_reference(carrier, target, start, &mut Quiet));
        result?;
        assert_eq!(allocations, 0);
        store.enter_scope(parent);
        assert_eq!(store.get(view), expected);
        store.enter_scope(child);
    }
    store.enter_scope(parent);
    assert_eq!(store.bindings().input(value).source, Some(second.id()));
    Ok(())
}

#[test]
fn reference_field_assembly_retains_unpublished_carrier_across_retargets() -> Result<(), BindError>
{
    let mut store = Store::new();
    let parent = store.scope();
    let shape = Kind::Bundle(vec![("value".into(), Kind::Reference(Box::new(scalar())))]);
    let bundle = store.add_shaped_input(NodeId(1), shape.clone(), false);
    let child = store.child_scope(NodeId(1));
    store.enter_scope(child);
    let target = store.add_output::<i64>(NodeId(0));
    let carrier = store.add_reference(NodeId(1), ScalarType::I64, false);
    store.enter_scope(parent);
    let value = store.bindings().fixed_input(bundle, 0);
    store.prepare_owned_view(value, carrier)?;
    let assembly = store.assemble_owned_view(bundle)?;
    let expected = store.items_reference(shape, vec![store.reference(carrier)])?;
    assert_eq!(assembly, expected);
    assert!(!store.bindings().valid(value));
    assert_eq!(store.bindings().input(value).source, Some(carrier));
    store.reserve_reference_target(target.id(), 1);
    store.enter_scope(child);
    for designation in [
        store.reference(target.id()),
        Reference::default(),
        store.reference(target.id()),
    ] {
        let (result, allocations) = count_in(|| {
            store.set_reference(carrier, designation, EngineTime::MIN_START, &mut Quiet)
        });
        result?;
        assert_eq!(allocations, 0);
        assert!(store.bindings().valid(value));
        assert_eq!(store.bindings().input(value).source, Some(carrier));
        assert_eq!(store.bindings().input_reference(value), designation);
        assert_eq!(store.bindings().input_reference(bundle), assembly);
    }
    Ok(())
}
