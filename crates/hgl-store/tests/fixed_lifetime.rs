//! Restoring stopped writers and rejecting stale compound attachments.
use hgl_store::{BindError, Kind, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
fn at(n: i64) -> EngineTime {
    EngineTime::from_micros(n)
}
fn shape() -> Kind {
    Kind::List(
        Box::new(Kind::List(Box::new(Kind::Ts(ScalarType::I64)), 2)),
        2,
    )
}

#[test]
fn stopped_writer_is_replaced_with_a_fresh_owned_subtree() -> Result<(), BindError> {
    let mut store = Store::new();
    let root = store.scope();
    let dict = store.add_shaped_output(NodeId(0), Kind::Dictionary(Box::new(shape())));
    let scope = store.child_scope(NodeId(0));
    store.enter_scope(scope);
    let old = store.add_shaped_output(NodeId(0), shape());
    let inner = store.bindings().fixed_output(old, 0);
    let old_leaf = store.bindings().fixed_output(inner, 0);
    let saved = store.reference(old_leaf);
    store.enter_scope(root);
    store.attach_shaped(dict, 0, store.reference(old), at(1), &mut Quiet)?;
    store.release_scope(scope, at(1), &mut Quiet);
    let new = store.get_or_create_shaped(dict, 0, at(1), &mut Quiet);
    assert_ne!(new, old);
    assert!(store.bindings().resolve(saved).is_some());
    store.begin_cycle(at(2));
    assert!(store.bindings().resolve(saved).is_none());
    let inner = store.bindings().fixed_output(new, 0);
    let leaf = store.scalar_output::<i64>(store.bindings().fixed_output(inner, 0))?;
    store.set(leaf, 7, at(2), NodeId(0), &mut Quiet);
    assert_eq!(store.output_value(leaf), Some(7));
    Ok(())
}
#[test]
fn dead_compound_output_cannot_be_reattached() {
    let mut store = Store::new();
    let dict = store.add_shaped_output(NodeId(0), Kind::Dictionary(Box::new(shape())));
    let old = store.get_or_create_shaped(dict, 0, at(1), &mut Quiet);
    let saved = store.reference(old);
    store.remove_shaped(dict, 0, at(1), &mut Quiet);
    store.begin_cycle(at(2));
    let counts = store.bindings().storage_counts();
    assert!(
        store
            .attach_shaped(dict, 0, saved, at(2), &mut Quiet)
            .is_err()
    );
    assert_eq!(store.bindings().child_output(dict, 0), None);
    assert_eq!(store.bindings().storage_counts(), counts);
    let replacement = store.get_or_create_shaped(dict, 1, at(2), &mut Quiet);
    assert_eq!(replacement, old, "exercise reuse of the expired slot");
    assert_eq!(
        store.attach_shaped(dict, 0, saved, at(2), &mut Quiet),
        Err(BindError::InvalidReference)
    );
    assert_eq!(store.bindings().child_output(dict, 0), None);
}

#[test]
fn a_new_child_scope_cannot_change_the_rank_of_a_retained_reference() -> Result<(), BindError> {
    let mut store = Store::new();
    let root = store.scope();
    let input = store.add_input::<i64>(NodeId(2), true);
    let old_scope = store.child_scope(NodeId(0));
    store.enter_scope(old_scope);
    let output = store.add_output::<i64>(NodeId(0));
    store.set(output, 7, at(1), NodeId(0), &mut Quiet);
    let saved = store.reference(output.id());
    store.enter_scope(root);
    store.release_scope(old_scope, at(1), &mut Quiet);
    store.begin_cycle(at(1));
    let _new_scope = store.child_scope(NodeId(3));
    store.sample(input.id(), saved, at(1), &mut Quiet)?;
    assert_eq!(store.get(input), 7);
    store.begin_cycle(at(2));
    assert!(!store.valid(input));
    assert!(store.bindings().resolve(saved).is_none());
    Ok(())
}
