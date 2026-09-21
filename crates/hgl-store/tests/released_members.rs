//! TS-11/TS-23: attached child outputs cannot outlive their graph as live members.
use hgl_store::{BindError, Store, Wake};
use hgl_types::{EngineTime, NodeId};
#[derive(Default)]
struct Wakes(Vec<NodeId>);
impl Wake for Wakes {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}
fn at(cycle: i64) -> EngineTime {
    EngineTime::from_micros(cycle + 1)
}
#[test]
fn release_removes_attached_members_before_their_slots_can_be_reused() -> Result<(), BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let parent = store.add_dictionary::<i64>(NodeId(0));
    let input = store.add_dictionary_input::<i64>(NodeId(1), true);
    store.bind(input.id(), parent.id())?;
    let root = store.scope();
    let scope = store.child_scope(NodeId(0));
    store.enter_scope(scope);
    let child = store.add_output::<i64>(NodeId(0));
    store.set(child, 7, at(0), NodeId(0), &mut wakes);
    store.enter_scope(root);
    store.attach(parent, 42, child, at(0), &mut wakes)?;
    let saved = store.reference(child.id());
    store.begin_cycle(at(1));
    wakes.0.clear();
    store.release_scope(scope, at(1), &mut wakes);
    assert_eq!(store.bindings().keys(input.id()).count(), 0);
    assert_eq!(
        store
            .bindings()
            .removed_keys(input.id())
            .collect::<Vec<_>>(),
        vec![42]
    );
    assert_eq!(wakes.0, vec![NodeId(1)]);
    let removed = store
        .removed_child(input, 42)
        .ok_or(BindError::ShapeMismatch)?;
    assert_eq!(store.get(removed), 7);
    assert!(store.bindings().resolve(saved).is_some());
    store.begin_cycle(at(2));
    assert!(store.removed_child(input, 42).is_none());
    assert!(store.bindings().resolve(saved).is_none());
    let unrelated = store.add_output::<i64>(NodeId(2));
    assert_eq!(unrelated.id(), child.id(), "exercise actual slot reuse");
    store.set(unrelated, 99, at(2), NodeId(2), &mut wakes);
    let late = store.add_dictionary_input::<i64>(NodeId(3), true);
    store.bind(late.id(), parent.id())?;
    assert!(store.child(late, 42).is_none());
    let fresh = store.get_or_create(parent, 42, at(2), &mut wakes);
    assert_ne!(fresh.id(), unrelated.id());
    assert_eq!(store.output_value(fresh), None);
    Ok(())
}

#[test]
fn releasing_old_scope_does_not_remove_a_same_cycle_replacement() -> Result<(), BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let parent = store.add_dictionary::<i64>(NodeId(0));
    let root = store.scope();
    let old_scope = store.child_scope(NodeId(0));
    store.enter_scope(old_scope);
    let old = store.add_output::<i64>(NodeId(0));
    store.set(old, 7, at(0), NodeId(0), &mut wakes);
    store.enter_scope(root);
    store.attach(parent, 42, old, at(0), &mut wakes)?;
    store.begin_cycle(at(1));
    store.remove(parent, 42, at(1), &mut wakes);
    let new_scope = store.child_scope(NodeId(0));
    store.enter_scope(new_scope);
    let new = store.add_output::<i64>(NodeId(0));
    store.set(new, 9, at(1), NodeId(0), &mut wakes);
    store.enter_scope(root);
    store.attach(parent, 42, new, at(1), &mut wakes)?;
    store.release_scope(old_scope, at(1), &mut wakes);
    store.begin_cycle(at(2));
    assert_eq!(
        store.bindings().child_output(parent.id(), 42),
        Some(new.id())
    );
    assert_eq!(store.output_value(new), Some(9));
    Ok(())
}

#[test]
fn expired_reference_detaches_dictionary_views_without_retiring_the_target() -> Result<(), BindError>
{
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let child = store.get_or_create(dictionary, 42, at(0), &mut wakes);
    store.set(child, 7, at(0), NodeId(0), &mut wakes);
    let root = store.scope();
    let scope = store.child_scope(NodeId(1));
    store.enter_scope(scope);
    let reference = store.add_reference(NodeId(0), hgl_types::ScalarType::I64, true);
    store.set_reference(
        reference,
        store.reference(dictionary.id()),
        at(0),
        &mut wakes,
    )?;
    store.enter_scope(root);
    let input = store.add_dictionary_input::<i64>(NodeId(2), true);
    store.follow(input.id(), reference, at(0), &mut wakes)?;
    store.begin_cycle(at(1));
    store.release_scope(scope, at(1), &mut wakes);
    assert!(store.child(input, 42).is_some());
    store.begin_cycle(at(2));
    assert!(!store.input_valid(input.id()));
    assert!(store.child(input, 42).is_none());
    assert_eq!(store.bindings().keys(input.id()).count(), 0);
    assert_eq!(store.bindings().storage_counts()[3], 0);
    assert_eq!(store.output_value(child), Some(7));
    Ok(())
}
