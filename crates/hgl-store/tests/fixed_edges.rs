//! Rebinding admission, stable projections, scope expiry and bounded churn.
use hgl_store::{BindError, Kind, Reference, Store, Wake};
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
        Box::new(Kind::List(Box::new(Kind::Scalar(ScalarType::I64)), 2)),
        2,
    )
}

#[test]
fn rebind_rejects_shape_and_backward_descendants_atomically() -> Result<(), BindError> {
    let mut store = Store::new();
    let shape = shape();
    let good = store.add_shaped_output(NodeId(0), shape.clone());
    let late = store.add_shaped_output(NodeId(3), shape.clone());
    let wrong = store.add_shaped_output(
        NodeId(0),
        Kind::List(Box::new(Kind::Scalar(ScalarType::I64)), 2),
    );
    let input = store.add_shaped_input(NodeId(2), shape.clone(), true);
    let r = store.add_shaped_output(NodeId(1), Kind::Reference(Box::new(shape.clone())));
    store.follow(input, r, at(1), &mut Quiet)?;
    store.set_reference(r, store.reference(good), at(1), &mut Quiet)?;
    let children = [
        store.bindings().fixed_input(input, 0),
        store.bindings().fixed_input(input, 1),
    ];
    let before = store.bindings().storage_counts();
    let mixed = store.items_reference(
        shape,
        vec![
            store.reference(store.bindings().fixed_output(good, 0)),
            store.reference(store.bindings().fixed_output(late, 1)),
        ],
    )?;
    assert_eq!(
        store.set_reference(r, mixed, at(2), &mut Quiet),
        Err(BindError::BackwardReference)
    );
    assert_eq!(
        store.sample(input, store.reference(wrong), at(2), &mut Quiet),
        Err(BindError::ShapeMismatch)
    );
    assert_eq!(store.bindings().input(input).source, Some(good));
    assert_eq!(store.bindings().storage_counts(), before);
    store.sample(input, Reference::default(), at(3), &mut Quiet)?;
    assert_eq!(store.bindings().fixed_input(input, 0), children[0]);
    store.unbind(input);
    store.bind(input, good)?;
    assert_eq!(store.bindings().fixed_input(input, 1), children[1]);
    assert_eq!(
        store.bindings().input(children[0]).source,
        Some(store.bindings().fixed_output(good, 0))
    );
    Ok(())
}

#[test]
fn expiry_clears_assembled_ancestors() -> Result<(), BindError> {
    let mut store = Store::new();
    let root = store.scope();
    let shape = shape();
    let input = store.add_shaped_input(NodeId(2), shape.clone(), true);
    let scope = store.child_scope(NodeId(0));
    store.enter_scope(scope);
    let output = store.add_shaped_output(NodeId(0), shape.clone());
    let left = store.bindings().fixed_output(output, 0);
    let leaf = store.scalar_output::<i64>(store.bindings().fixed_output(left, 0))?;
    store.set(leaf, 7, at(1), NodeId(0), &mut Quiet);
    store.enter_scope(root);
    let items = store.items_reference(shape, vec![store.reference(left), Reference::default()])?;
    store.sample(input, items, at(2), &mut Quiet)?;
    let child = store.bindings().fixed_input(input, 0);
    assert_eq!(store.bindings().last_modified(child), at(2));
    store.release_scope(scope, at(3), &mut Quiet);
    assert!(store.input_valid(input));
    store.begin_cycle(at(4));
    assert!(!store.input_valid(input));
    assert_eq!(store.bindings().last_modified(input), EngineTime::NEVER);
    assert_eq!(store.bindings().last_modified(child), EngineTime::NEVER);
    assert!(
        store
            .bindings()
            .resolve(store.reference(leaf.id()))
            .is_none()
    );
    Ok(())
}

#[test]
fn compound_churn_reuses_every_descendant_and_never_revives_saved_handles() -> Result<(), BindError>
{
    let mut store = Store::new();
    let dict = store.add_shaped_output(NodeId(0), Kind::Dictionary(Box::new(shape())));
    let input = store.add_shaped_input(NodeId(2), Kind::Dictionary(Box::new(shape())), true);
    store.bind(input, dict)?;
    let mut bound = [0; 4];
    let mut saved = Vec::new();
    for cycle in 1..=100 {
        let now = at(cycle * 2);
        store.begin_cycle(now);
        for r in &saved {
            assert!(store.bindings().resolve(*r).is_none());
        }
        let child = store.get_or_create_shaped(dict, 0, now, &mut Quiet);
        let inner = store.bindings().fixed_output(child, 0);
        let leaf = store.scalar_output::<i64>(store.bindings().fixed_output(inner, 0))?;
        store.set(leaf, cycle, now, NodeId(0), &mut Quiet);
        saved.push(store.reference(leaf.id()));
        store.remove_shaped(dict, 0, now, &mut Quiet);
        let restored = store.get_or_create_shaped(dict, 0, now, &mut Quiet);
        assert_eq!(restored, child);
        store.remove_shaped(dict, 0, now, &mut Quiet);
        store.begin_cycle(at(cycle * 2 + 1));
        if cycle == 3 {
            bound = store.bindings().storage_counts();
        }
        if cycle > 3 {
            assert_eq!(store.bindings().storage_counts(), bound);
        }
    }
    Ok(())
}

#[test]
fn sampling_valid_owned_structure_with_invalid_children_preserves_root_time()
-> Result<(), BindError> {
    let mut store = Store::new();
    let shape = Kind::List(Box::new(Kind::Scalar(ScalarType::I64)), 2);
    let output = store.add_shaped_output(NodeId(0), shape.clone());
    let leaf = store.scalar_output::<i64>(store.bindings().fixed_output(output, 0))?;
    store.set(leaf, 7, at(1), NodeId(0), &mut Quiet);
    store.invalidate(leaf.id(), at(2), &mut Quiet);
    let input = store.add_shaped_input(NodeId(2), shape, true);
    store.sample(input, store.reference(output), at(3), &mut Quiet)?;
    assert!(store.input_valid(input));
    assert!(!store.bindings().all_valid(input));
    assert_eq!(store.bindings().last_modified(input), at(3));
    assert_eq!(
        store
            .bindings()
            .last_modified(store.bindings().fixed_input(input, 0)),
        EngineTime::NEVER
    );
    assert_eq!(store.bindings().output(output).modified_at, at(2));
    Ok(())
}

#[test]
fn designation_producer_must_also_precede_its_follower() -> Result<(), BindError> {
    let mut store = Store::new();
    let output = store.add_shaped_output(NodeId(0), shape());
    let input = store.add_shaped_input(NodeId(2), shape(), true);
    let reference = store.add_shaped_output(NodeId(3), Kind::Reference(Box::new(shape())));
    store.bind(input, output)?;
    store.set_reference(reference, store.reference(output), at(1), &mut Quiet)?;
    assert_eq!(
        store.follow(input, reference, at(1), &mut Quiet),
        Err(BindError::BackwardReference)
    );
    assert_eq!(store.bindings().input(input).source, Some(output));
    Ok(())
}

#[test]
fn sampling_dictionary_of_fixed_children_stamps_every_valid_level() -> Result<(), BindError> {
    let mut store = Store::new();
    let kind = Kind::Dictionary(Box::new(shape()));
    let output = store.add_shaped_output(NodeId(0), kind.clone());
    let child = store.get_or_create_shaped(output, 0, at(1), &mut Quiet);
    let inner = store.bindings().fixed_output(child, 0);
    let leaf = store.scalar_output::<i64>(store.bindings().fixed_output(inner, 0))?;
    store.set(leaf, 7, at(1), NodeId(0), &mut Quiet);
    let input = store.add_shaped_input(NodeId(2), kind, true);
    store.sample(input, store.reference(output), at(2), &mut Quiet)?;
    let child = store
        .bindings()
        .child_input(input, 0)
        .ok_or(BindError::ShapeMismatch)?;
    let inner = store.bindings().fixed_input(child, 0);
    let leaf = store.bindings().fixed_input(inner, 0);
    for id in [input, child, inner, leaf] {
        assert_eq!(store.bindings().last_modified(id), at(2));
    }
    assert_eq!(store.bindings().output(output).modified_at, at(1));
    Ok(())
}
