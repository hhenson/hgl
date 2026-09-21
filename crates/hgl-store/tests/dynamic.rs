//! Accepted TS-11, TS-14–TS-19 and TS-23 observations.
use hgl_store::{Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
#[derive(Default)]
struct Wakes(Vec<NodeId>);
impl Wake for Wakes {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}
fn at(t: i64) -> EngineTime {
    EngineTime::from_micros(t + 1)
}

#[test]
fn reference_samples_without_changing_producer_time_and_follows_passively()
-> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let a = store.add_output::<i64>(NodeId(0));
    let b = store.add_output::<i64>(NodeId(1));
    let reference = store.add_reference(NodeId(2), ScalarType::I64, false);
    let input = store.add_input::<i64>(NodeId(3), false);
    store.follow(input.id(), reference, at(0), &mut wakes)?;
    let mut observations = Vec::new();
    for t in 0..8 {
        store.begin_cycle(at(t));
        match t {
            0 => {
                store.set(a, 7, at(t), NodeId(0), &mut wakes);
                store.set(b, 20, at(t), NodeId(1), &mut wakes);
            }
            2 => store.set(a, 8, at(t), NodeId(0), &mut wakes),
            3 => store.set(b, 21, at(t), NodeId(1), &mut wakes),
            6 => store.set(a, 9, at(t), NodeId(0), &mut wakes),
            _ => {}
        }
        let select = match t {
            0 | 5 => Some(Reference::default()),
            1 | 6 => Some(store.reference(a.id())),
            3 => Some(store.reference(b.id())),
            _ => None,
        };
        if let Some(select) = select {
            store.set_reference(reference, select, at(t), &mut wakes)?;
        }
        if t == 4 {
            store.follow(input.id(), reference, at(t), &mut wakes)?;
        }
        observations.push((
            store.valid(input).then(|| store.get(input)),
            store.modified(input, at(t)),
            store.last_modified(input),
        ));
        if t == 1 {
            assert_eq!(store.bindings().output(a.id()).modified_at, at(0));
        }
    }
    assert_eq!(
        observations,
        vec![
            (None, false, EngineTime::NEVER),
            (Some(7), true, at(1)),
            (Some(8), true, at(2)),
            (Some(21), true, at(3)),
            (Some(21), false, at(3)),
            (None, false, EngineTime::NEVER),
            (Some(9), true, at(6)),
            (Some(9), false, at(6))
        ]
    );
    assert!(wakes.0.is_empty(), "TS-8: passive following must not wake");
    Ok(())
}

#[test]
fn dictionary_membership_is_independent_of_child_publication() -> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let input = store.add_dictionary_input::<i64>(NodeId(1), true);
    store.bind(input.id(), dictionary.id())?;
    let mut rows = Vec::new();
    for t in 0..8 {
        store.begin_cycle(at(t));
        match t {
            1 => {
                store.get_or_create(dictionary, 0, at(t), &mut wakes);
            }
            2 | 6 => {
                let child = store.get_or_create(dictionary, 0, at(t), &mut wakes);
                store.set(
                    child,
                    if t == 2 { 7 } else { 9 },
                    at(t),
                    NodeId(0),
                    &mut wakes,
                );
            }
            3 => {
                let child = store.get_or_create(dictionary, 0, at(t), &mut wakes);
                store.invalidate(child.id(), at(t), &mut wakes);
            }
            4 => store.remove(dictionary, 0, at(t), &mut wakes),
            _ => {}
        }
        rows.push((
            store.input_valid(input.id()),
            store.bindings().all_valid(input.id()),
            store.bindings().modified(input.id(), at(t)),
            store.bindings().added_keys(input.id()).collect::<Vec<_>>(),
            store
                .bindings()
                .removed_keys(input.id())
                .collect::<Vec<_>>(),
        ));
    }
    assert_eq!(
        rows,
        vec![
            (false, false, false, vec![], vec![]),
            (true, false, true, vec![0], vec![]),
            (true, true, true, vec![], vec![]),
            (true, false, true, vec![], vec![]),
            (true, true, true, vec![], vec![0]),
            (true, true, false, vec![], vec![]),
            (true, true, true, vec![0], vec![]),
            (true, true, false, vec![], vec![])
        ]
    );
    Ok(())
}

#[test]
fn restored_child_preserves_identity_then_saved_reference_expires_without_further_mutation()
-> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let input = store.add_dictionary_input::<i64>(NodeId(1), true);
    store.bind(input.id(), dictionary.id())?;
    let child = store.get_or_create(dictionary, 0, at(0), &mut wakes);
    store.set(child, 7, at(0), NodeId(0), &mut wakes);
    let saved = store.reference(child.id());
    store.remove(dictionary, 0, at(1), &mut wakes);
    let restored = store.get_or_create(dictionary, 0, at(1), &mut wakes);
    assert_eq!(child.id(), restored.id());
    assert_eq!(store.output_value(restored), Some(7));
    assert_eq!(store.bindings().output(restored.id()).modified_at, at(0));
    assert_eq!(store.bindings().added_keys(input.id()).count(), 0);
    assert_eq!(store.bindings().removed_keys(input.id()).count(), 0);
    store.remove(dictionary, 0, at(2), &mut wakes);
    assert_eq!(store.output_value(child), Some(7));
    assert!(store.bindings().resolve(saved).is_some());
    let view = store
        .removed_child(input, 0)
        .ok_or(hgl_store::BindError::UnknownInput(input.id()))?;
    assert_eq!(store.get(view), 7);
    store.begin_cycle(at(3));
    assert!(store.bindings().resolve(saved).is_none());
    assert!(store.removed_child(input, 0).is_none());
    let new = store.get_or_create(dictionary, 0, at(4), &mut wakes);
    store.set(new, 9, at(4), NodeId(0), &mut wakes);
    assert_eq!(
        child.id(),
        new.id(),
        "test must exercise physical slot reuse"
    );
    assert!(store.bindings().resolve(saved).is_none());
    let followed = store.add_input::<i64>(NodeId(2), true);
    store.sample(followed.id(), saved, at(5), &mut wakes)?;
    assert!(!store.valid(followed));
    Ok(())
}

#[test]
fn dictionary_rebind_samples_overlapping_children_and_withdrawal_remains_readable()
-> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let a = store.add_dictionary::<i64>(NodeId(0));
    let b = store.add_dictionary::<i64>(NodeId(1));
    for (dictionary, owner, values) in [
        (a, NodeId(0), [(0, 1), (2, 9)]),
        (b, NodeId(1), [(1, 2), (2, 3)]),
    ] {
        for (key, value) in values {
            let child = store.get_or_create(dictionary, key, at(0), &mut wakes);
            store.set(child, value, at(0), owner, &mut wakes);
        }
    }
    let reference = store.add_reference(NodeId(2), ScalarType::I64, true);
    let input = store.add_dictionary_input::<i64>(NodeId(3), true);
    store.follow(input.id(), reference, at(0), &mut wakes)?;
    store.set_reference(reference, store.reference(a.id()), at(0), &mut wakes)?;
    store.begin_cycle(at(1));
    let y = store.get_or_create(b, 1, at(1), &mut wakes);
    store.set(y, 4, at(1), NodeId(1), &mut wakes);
    store.begin_cycle(at(2));
    store.set_reference(reference, store.reference(b.id()), at(2), &mut wakes)?;
    assert_eq!(
        store.bindings().added_keys(input.id()).collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        store
            .bindings()
            .removed_keys(input.id())
            .collect::<Vec<_>>(),
        vec![0]
    );
    for (key, value) in [(1, 4), (2, 3)] {
        let child = store
            .child(input, key)
            .ok_or(hgl_store::BindError::UnknownInput(input.id()))?;
        assert_eq!(store.get(child), value);
        assert_eq!(store.last_modified(child), at(2));
        assert!(store.modified(child, at(2)));
    }
    assert_eq!(
        store.get(
            store
                .removed_child(input, 0)
                .ok_or(hgl_store::BindError::UnknownInput(input.id()))?
        ),
        1
    );
    store.begin_cycle(at(3));
    store.set_reference(reference, Reference::default(), at(3), &mut wakes)?;
    assert!(!store.input_valid(input.id()));
    assert!(store.bindings().modified(input.id(), at(3)));
    assert_eq!(
        store.bindings().last_modified(input.id()),
        EngineTime::NEVER
    );
    assert_eq!(
        store
            .bindings()
            .removed_keys(input.id())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        store.get(
            store
                .removed_child(input, 1)
                .ok_or(hgl_store::BindError::UnknownInput(input.id()))?
        ),
        4
    );
    store.begin_cycle(at(4));
    store.set_reference(reference, store.reference(a.id()), at(4), &mut wakes)?;
    assert_eq!(
        store.bindings().added_keys(input.id()).collect::<Vec<_>>(),
        vec![0, 2]
    );
    Ok(())
}

#[test]
fn churn_reuses_ports_and_cannot_revive_a_saved_designation() -> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let input = store.add_dictionary_input::<i64>(NodeId(1), true);
    store.bind(input.id(), dictionary.id())?;
    let mut first = None;
    let mut maximum = [0; 4];
    for cycle in 0..1000 {
        store.begin_cycle(at(cycle));
        let child = store.get_or_create(dictionary, 0, at(cycle), &mut wakes);
        if let Some(reference) = first {
            assert!(store.bindings().resolve(reference).is_none());
        } else {
            first = Some(store.reference(child.id()));
        }
        store.set(child, cycle, at(cycle), NodeId(0), &mut wakes);
        // Several same-cycle removal/restoration pairs must queue one retirement.
        for _ in 0..3 {
            store.remove(dictionary, 0, at(cycle), &mut wakes);
            let restored = store.get_or_create(dictionary, 0, at(cycle), &mut wakes);
            assert_eq!(child.id(), restored.id());
        }
        store.remove(dictionary, 0, at(cycle), &mut wakes);
        let counts = store.bindings().storage_counts();
        if cycle == 2 {
            maximum = counts;
        }
        if cycle > 2 {
            assert_eq!(counts, maximum);
        }
    }
    store.begin_cycle(at(1000));
    let one = store.get_or_create(dictionary, 0, at(1000), &mut wakes);
    let two = store.get_or_create(dictionary, 1, at(1000), &mut wakes);
    assert_ne!(
        one.id(),
        two.id(),
        "a retired slot may enter the free list only once"
    );
    Ok(())
}

#[test]
fn rejected_backward_reference_preserves_all_followers() -> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let first = store.add_output::<i64>(NodeId(0));
    let late = store.add_output::<i64>(NodeId(3));
    let reference = store.add_reference(NodeId(1), ScalarType::I64, false);
    let before = store.add_input::<i64>(NodeId(2), true);
    let after = store.add_input::<i64>(NodeId(4), true);
    store.set(first, 7, at(0), NodeId(0), &mut wakes);
    store.set(late, 9, at(0), NodeId(3), &mut wakes);
    store.follow(after.id(), reference, at(0), &mut wakes)?;
    store.follow(before.id(), reference, at(0), &mut wakes)?;
    store.set_reference(reference, store.reference(first.id()), at(0), &mut wakes)?;
    assert_eq!(
        store.set_reference(reference, store.reference(late.id()), at(1), &mut wakes),
        Err(hgl_store::BindError::BackwardReference)
    );
    assert_eq!((store.get(before), store.get(after)), (7, 7));
    Ok(())
}

#[test]
#[should_panic(expected = "expired output handle")]
fn stale_writer_cannot_write_a_reused_child_slot() {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let old = store.get_or_create(dictionary, 0, at(0), &mut wakes);
    store.set(old, 7, at(0), NodeId(0), &mut wakes);
    store.remove(dictionary, 0, at(1), &mut wakes);
    store.begin_cycle(at(2));
    let new = store.get_or_create(dictionary, 1, at(2), &mut wakes);
    assert_eq!(old.id(), new.id());
    assert_eq!(store.output_value(new), None);
    store.set(old, 99, at(2), NodeId(0), &mut wakes);
}
#[test]
fn collection_invalidation_reaches_children_and_reference_invalidation_unbinds()
-> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let child = store.get_or_create(dictionary, 0, at(0), &mut wakes);
    store.set(child, 7, at(0), NodeId(0), &mut wakes);
    let reference = store.add_reference(NodeId(1), ScalarType::I64, true);
    let input = store.add_dictionary_input::<i64>(NodeId(2), true);
    store.follow(input.id(), reference, at(0), &mut wakes)?;
    store.set_reference(
        reference,
        store.reference(dictionary.id()),
        at(0),
        &mut wakes,
    )?;
    store.begin_cycle(at(1));
    store.invalidate(dictionary.id(), at(1), &mut wakes);
    assert!(!store.input_valid(input.id()));
    assert_eq!(store.output_value(child), None);
    store.begin_cycle(at(2));
    store.set(child, 9, at(2), NodeId(0), &mut wakes);
    assert!(store.bindings().all_valid(input.id()));
    store.begin_cycle(at(3));
    store.invalidate(reference, at(3), &mut wakes);
    assert!(!store.input_valid(input.id()));
    assert_eq!(
        store
            .bindings()
            .removed_keys(input.id())
            .collect::<Vec<_>>(),
        vec![0]
    );
    Ok(())
}

#[test]
fn switching_and_unbinding_remove_old_target_notifications() -> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let a = store.add_output::<i64>(NodeId(0));
    let b = store.add_output::<i64>(NodeId(1));
    let input = store.add_input::<i64>(NodeId(2), true);
    store.set(a, 7, at(0), NodeId(0), &mut wakes);
    store.set(b, 9, at(0), NodeId(1), &mut wakes);
    store.sample(input.id(), store.reference(a.id()), at(0), &mut wakes)?;
    store.begin_cycle(at(1));
    store.sample(input.id(), store.reference(b.id()), at(1), &mut wakes)?;
    store.begin_cycle(at(2));
    wakes.0.clear();
    store.set(a, 8, at(2), NodeId(0), &mut wakes);
    assert!(
        wakes.0.is_empty(),
        "old target must no longer wake its former consumer"
    );
    assert_eq!(store.get(input), 9);
    assert!(!store.modified(input, at(2)));
    store.set(b, 10, at(2), NodeId(1), &mut wakes);
    assert_eq!(wakes.0, vec![NodeId(2)]);
    store.unbind(input.id());
    store.begin_cycle(at(3));
    wakes.0.clear();
    store.set(b, 11, at(3), NodeId(1), &mut wakes);
    assert!(wakes.0.is_empty());
    assert!(!store.valid(input));
    assert_eq!(store.bindings().storage_counts()[3], 0);
    Ok(())
}

#[test]
fn expired_dictionary_drops_child_views_but_keeps_following_its_ref()
-> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let root = store.scope();
    let reference = store.add_reference(NodeId(0), ScalarType::I64, true);
    let input = store.add_dictionary_input::<i64>(NodeId(2), true);
    let scope = store.child_scope(NodeId(1));
    store.enter_scope(scope);
    let old = store.add_dictionary::<i64>(NodeId(0));
    let child = store.get_or_create(old, 0, at(0), &mut wakes);
    store.set(child, 7, at(0), NodeId(0), &mut wakes);
    store.enter_scope(root);
    store.follow(input.id(), reference, at(0), &mut wakes)?;
    store.set_reference(reference, store.reference(old.id()), at(0), &mut wakes)?;
    store.begin_cycle(at(1));
    store.release_scope(scope, at(1));
    assert!(store.input_valid(input.id()));
    store.begin_cycle(at(2));
    assert!(!store.input_valid(input.id()));
    assert_eq!(store.bindings().keys(input.id()).count(), 0);
    assert!(store.child(input, 0).is_none());
    let new = store.add_dictionary::<i64>(NodeId(1));
    let child = store.get_or_create(new, 1, at(2), &mut wakes);
    store.set(child, 9, at(2), NodeId(1), &mut wakes);
    store.set_reference(reference, store.reference(new.id()), at(2), &mut wakes)?;
    let view = store
        .child(input, 1)
        .ok_or(hgl_store::BindError::ShapeMismatch)?;
    assert_eq!(store.get(view), 9);
    Ok(())
}

#[test]
fn erased_scalar_observation_does_not_read_collection_or_ref_metadata_as_a_value()
-> Result<(), hgl_store::BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let child = store.get_or_create(dictionary, 0, at(0), &mut wakes);
    store.set(child, 7, at(0), NodeId(0), &mut wakes);
    let reference = store.add_reference(NodeId(1), ScalarType::I64, true);
    store.set_reference(
        reference,
        store.reference(dictionary.id()),
        at(0),
        &mut wakes,
    )?;
    assert_eq!(store.output_value_erased(dictionary.id()), None);
    assert_eq!(store.output_value_erased(reference), None);
    assert_eq!(
        store.output_value_erased(child.id()),
        Some(hgl_types::ScalarValue::I64(7))
    );
    Ok(())
}
