//! Interned structural storage follows scopes and current bindings, not copied handles.
use hgl_store::{BindError, Kind, Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn at(n: i64) -> EngineTime {
    EngineTime::from_micros(n)
}
fn scalar() -> Kind {
    Kind::Ts(ScalarType::I64)
}
fn shape() -> Kind {
    Kind::List(Box::new(scalar()), 2)
}
fn nested() -> Kind {
    Kind::List(Box::new(shape()), 1)
}
fn tree(store: &mut Store, left: Reference, right: Reference) -> Result<Reference, BindError> {
    let child = store.items_reference(shape(), vec![left, right])?;
    store.items_reference(nested(), vec![child])
}
#[test]
fn retired_assemblies_are_readable_then_stale_after_slot_reuse() -> Result<(), BindError> {
    let mut store = Store::new();
    let parent = store.scope();
    let view = store.add_shaped_input(NodeId(2), nested(), false);
    let mut saved = Reference::default();
    let mut bound = None;
    for cycle in 1..100 {
        store.begin_cycle(at(cycle));
        assert_eq!(store.bindings().assembly_counts()[1], 0);
        let scope = store.child_scope(NodeId(0));
        store.enter_scope(scope);
        let out = store.add_output::<i64>(NodeId(0));
        store.set(out, cycle, at(cycle), NodeId(0), &mut Quiet);
        let peer = store.reference(out.id());
        let current = tree(&mut store, peer, Reference::default())?;
        store.enter_scope(parent);
        let child = store.bindings().fixed_input(view, 0);
        let leaf = store.scalar_input::<i64>(store.bindings().fixed_input(child, 0))?;
        store.sample(view, current, at(cycle), &mut Quiet)?;
        assert_eq!(store.get(leaf), cycle);
        if cycle > 1 {
            assert_eq!(current.items, saved.items, "exercise reused assembly slots");
            assert_ne!(current, saved);
            store.sample(view, saved, at(cycle), &mut Quiet)?;
            assert!(
                !store.input_valid(view),
                "old generation must not read a replacement"
            );
            store.sample(view, current, at(cycle), &mut Quiet)?;
            assert_eq!(store.get(leaf), cycle);
        }
        store.release_scope(scope, at(cycle), &mut Quiet);
        assert_eq!(store.get(leaf), cycle, "removal-cycle read");
        store.unbind(view);
        saved = current;
        let counts = (
            store.bindings().storage_counts(),
            store.bindings().assembly_counts(),
        );
        if cycle == 3 {
            bound = Some(counts);
        } else if cycle > 3 {
            assert_eq!(Some(counts), bound);
        }
    }
    store.begin_cycle(at(100));
    assert_eq!(store.bindings().assembly_counts()[1], 0);
    Ok(())
}

#[test]
fn external_carrier_keeps_metadata_but_does_not_keep_child_endpoints() -> Result<(), BindError> {
    let mut store = Store::new();
    let parent = store.scope();
    let stable = store.add_output::<i64>(NodeId(0));
    store.set(stable, 3, at(1), NodeId(0), &mut Quiet);
    let carrier = store.add_shaped_output(NodeId(1), Kind::Reference(Box::new(nested())));
    let view = store.add_shaped_input(NodeId(2), nested(), false);
    store.follow(view, carrier, at(1), &mut Quiet)?;
    let scope = store.child_scope(NodeId(0));
    store.enter_scope(scope);
    let temporary = store.add_output::<i64>(NodeId(0));
    store.set(temporary, 7, at(1), NodeId(0), &mut Quiet);
    let refs = [
        store.reference(stable.id()),
        store.reference(temporary.id()),
    ];
    let value = tree(&mut store, refs[0], refs[1])?;
    store.enter_scope(parent);
    store.set_reference(carrier, value, at(1), &mut Quiet)?;
    let child = store.bindings().fixed_input(view, 0);
    let left = store.scalar_input::<i64>(store.bindings().fixed_input(child, 0))?;
    let right = store.scalar_input::<i64>(store.bindings().fixed_input(child, 1))?;
    store.release_scope(scope, at(1), &mut Quiet);
    assert_eq!(store.get(right), 7);
    store.begin_cycle(at(2));
    assert_eq!(store.get(left), 3);
    assert!(!store.valid(right));
    assert_eq!(store.bindings().reference_value(carrier), value);
    let later = store.add_shaped_input(NodeId(3), nested(), false);
    store.sample(later, value, at(2), &mut Quiet)?;
    assert!(store.input_valid(later));
    store.unbind(later);
    store.invalidate(carrier, at(3), &mut Quiet);
    assert!(!store.input_valid(view));
    assert_eq!(store.bindings().assembly_counts()[1], 0);
    Ok(())
}

#[test]
fn interning_is_shared_until_the_last_constructing_scope_retires() -> Result<(), BindError> {
    let mut store = Store::new();
    let parent = store.scope();
    let out = store.add_output::<i64>(NodeId(0));
    let peer = store.reference(out.id());
    let first = store.child_scope(NodeId(1));
    store.enter_scope(first);
    let a = tree(&mut store, peer, Reference::default())?;
    store.enter_scope(parent);
    let second = store.child_scope(NodeId(1));
    store.enter_scope(second);
    let b = tree(&mut store, peer, Reference::default())?;
    assert_eq!(
        a, b,
        "equal designations have canonical identity across scopes"
    );
    assert_eq!(store.bindings().assembly_counts(), [2, 2]);
    store.enter_scope(parent);
    store.release_scope(first, at(1), &mut Quiet);
    store.begin_cycle(at(2));
    assert_eq!(store.bindings().assembly_counts(), [2, 2]);
    store.release_scope(second, at(2), &mut Quiet);
    store.begin_cycle(at(3));
    assert_eq!(store.bindings().assembly_counts(), [2, 0]);
    Ok(())
}

#[test]
fn replacing_external_designations_does_not_retain_previous_lifetimes() -> Result<(), BindError> {
    let mut store = Store::new();
    let parent = store.scope();
    let carrier = store.add_shaped_output(NodeId(1), Kind::Reference(Box::new(nested())));
    let view = store.add_shaped_input(NodeId(2), nested(), false);
    store.follow(view, carrier, at(1), &mut Quiet)?;
    let mut bound = None;
    for cycle in 1..100 {
        store.begin_cycle(at(cycle));
        let scope = store.child_scope(NodeId(0));
        store.enter_scope(scope);
        let out = store.add_output::<i64>(NodeId(0));
        store.set(out, cycle, at(cycle), NodeId(0), &mut Quiet);
        let peer = store.reference(out.id());
        let r = tree(&mut store, peer, Reference::default())?;
        store.enter_scope(parent);
        store.set_reference(carrier, r, at(cycle), &mut Quiet)?;
        store.release_scope(scope, at(cycle), &mut Quiet);
        let counts = store.bindings().assembly_counts();
        if cycle == 3 {
            bound = Some(counts);
        } else if cycle > 3 {
            assert_eq!(Some(counts), bound);
        }
    }
    store.begin_cycle(at(100));
    store.set_reference(carrier, Reference::default(), at(100), &mut Quiet)?;
    assert_eq!(store.bindings().assembly_counts()[1], 0);
    Ok(())
}

#[test]
fn enclosing_assembly_retains_borrowed_children_until_carrier_expiry() -> Result<(), BindError> {
    let mut store = Store::new();
    let parent = store.scope();
    let out = store.add_output::<i64>(NodeId(0));
    store.set(out, 9, at(1), NodeId(0), &mut Quiet);
    let peer = store.reference(out.id());
    let first = store.child_scope(NodeId(1));
    store.enter_scope(first);
    let inner = store.items_reference(shape(), vec![peer, Reference::default()])?;
    store.enter_scope(parent);
    let second = store.child_scope(NodeId(2));
    store.enter_scope(second);
    let outer = store.items_reference(nested(), vec![inner])?;
    store.enter_scope(parent);
    let third = store.child_scope(NodeId(3));
    store.enter_scope(third);
    let carrier = store.add_shaped_output(NodeId(0), Kind::Reference(Box::new(nested())));
    store.set_reference(carrier, outer, at(1), &mut Quiet)?;
    store.enter_scope(parent);
    store.release_scope(first, at(1), &mut Quiet);
    store.release_scope(second, at(1), &mut Quiet);
    store.begin_cycle(at(2));
    let view = store.add_shaped_input(NodeId(4), nested(), false);
    store.follow(view, carrier, at(2), &mut Quiet)?;
    let child = store.bindings().fixed_input(view, 0);
    let leaf = store.scalar_input::<i64>(store.bindings().fixed_input(child, 0))?;
    assert_eq!(store.get(leaf), 9);
    assert_eq!(store.bindings().assembly_counts()[1], 2);
    store.release_scope(third, at(2), &mut Quiet);
    assert_eq!(store.get(leaf), 9, "carrier removal-cycle read");
    store.begin_cycle(at(3));
    assert!(!store.input_valid(view));
    assert_eq!(store.bindings().assembly_counts()[1], 0);
    Ok(())
}
