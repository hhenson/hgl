//! Static structural projections, sparse leaf updates, and typed member factories.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{
    BindError, Store, Wake,
    shapes::{Field, Fixed, Input, Map, Output, Shape},
};
use hgl_types::{EngineTime, NodeId, TsType};
use std::sync::atomic::{AtomicUsize, Ordering};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
struct Pair;
static SCHEMAS: AtomicUsize = AtomicUsize::new(0);
impl Shape for Pair {
    fn shape() -> TsType {
        SCHEMAS.fetch_add(1, Ordering::Relaxed);
        TsType::Bundle(vec![
            ("amount".into(), i64::shape()),
            ("text".into(), Fixed::<String, 2>::shape()),
        ])
    }
}
impl Field<0> for Pair {
    type Child = i64;
}
impl Field<1> for Pair {
    type Child = Fixed<String, 2>;
}
fn at(n: i64) -> EngineTime {
    EngineTime::from_micros(n)
}
#[test]
fn nested_projections_retain_shape_without_hot_schema_calls_or_allocations() -> Result<(), BindError>
{
    let mut store = Store::new();
    let root = store.add_shaped_output(NodeId(0), Pair::shape());
    let target = store.add_shaped_input(NodeId(1), Pair::shape(), true);
    store.bind(target, root)?;
    let output = Output::<Pair>::bind(store.bindings(), root)?;
    let input = Input::<Pair>::bind(store.bindings(), target)?;
    assert!(Input::<i64>::bind(store.bindings(), target).is_err());
    assert!(Output::<Fixed<i64, 2>>::bind(store.bindings(), root).is_err());
    let leaf = output.field::<0>(store.bindings());
    let text = output
        .field::<1>(store.bindings())
        .index(store.bindings(), 1);
    store.set(
        Store::prepared_output(leaf),
        42,
        at(1),
        NodeId(0),
        &mut Quiet,
    );
    store.set(
        Store::prepared_output(text),
        "large".repeat(4096),
        at(1),
        NodeId(0),
        &mut Quiet,
    );
    let schemas = SCHEMAS.load(Ordering::Relaxed);
    let ((), allocations) = count_in(|| {
        for _ in 0..10_000 {
            let alias = input;
            let amount = Store::prepared_input(alias.field::<0>(store.bindings()));
            let label = Store::prepared_input(
                alias
                    .field::<1>(store.bindings())
                    .index(store.bindings(), 1),
            );
            assert_eq!(store.get(amount), 42);
            assert_eq!(store.get_ref(label).len(), 5 * 4096);
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(SCHEMAS.load(Ordering::Relaxed), schemas);
    let untouched = input
        .field::<1>(store.bindings())
        .index(store.bindings(), 0);
    assert!(!store.input_valid(untouched.id()));
    store.set(
        Store::prepared_output(leaf),
        42,
        at(2),
        NodeId(0),
        &mut Quiet,
    );
    assert!(
        store
            .bindings()
            .modified(input.field::<0>(store.bindings()).id(), at(2))
    );
    assert!(
        !store
            .bindings()
            .modified(input.field::<1>(store.bindings()).id(), at(2))
    );
    Ok(())
}
#[test]
fn typed_member_factory_runs_only_for_new_storage_and_sparse_updates_preserve_siblings()
-> Result<(), BindError> {
    let mut store = Store::new();
    let root = store.add_shaped_output(NodeId(0), Map::<Fixed<i64, 2>>::shape());
    let input = store.add_shaped_input(NodeId(1), Map::<Fixed<i64, 2>>::shape(), true);
    store.bind(input, root)?;
    let output = Output::<Map<Fixed<i64, 2>>>::bind(store.bindings(), root)?;
    let input = Input::<Map<Fixed<i64, 2>>>::bind(store.bindings(), input)?;
    let mut made = 0;
    for (time, index, value) in [(1, 0, 10), (2, 1, 20), (3, 0, 10)] {
        store.get_or_create_with(root, 7, at(time), &mut Quiet, |store, owner| {
            made += 1;
            let a = store.add_output::<i64>(owner).id();
            let b = store.add_output::<i64>(owner).id();
            store.add_prepared_output(owner, Fixed::<i64, 2>::shape(), vec![a, b])
        });
        let child = output
            .member(store.bindings(), 7)
            .ok_or(BindError::ShapeMismatch)?;
        let leaf = Store::prepared_output(child.index(store.bindings(), index));
        store.set(leaf, value, at(time), NodeId(0), &mut Quiet);
    }
    assert_eq!(made, 1);
    let child = input
        .member(store.bindings(), 7)
        .ok_or(BindError::ShapeMismatch)?;
    let a = Store::prepared_input(child.index(store.bindings(), 0));
    let b = Store::prepared_input(child.index(store.bindings(), 1));
    assert_eq!(store.get(a), 10);
    assert_eq!(store.get(b), 20);
    assert!(store.modified(a, at(3)));
    assert!(!store.modified(b, at(3)));
    store.remove_shaped(root, 7, at(4), &mut Quiet);
    assert!(input.member(store.bindings(), 7).is_none());
    assert_eq!(
        store
            .bindings()
            .removed_keys(input.id())
            .collect::<Vec<_>>(),
        vec![7]
    );
    Ok(())
}

#[test]
#[cfg(debug_assertions)]
fn expired_parent_token_cannot_project_into_reused_descendants() -> Result<(), BindError> {
    let mut store = Store::new();
    let root = store.add_shaped_output(NodeId(0), Map::<Fixed<i64, 1>>::shape());
    let output = Output::<Map<Fixed<i64, 1>>>::bind(store.bindings(), root)?;
    store.get_or_create_with(root, 1, at(1), &mut Quiet, |store, owner| {
        let child = store.add_output::<i64>(owner).id();
        store.add_prepared_output(owner, Fixed::<i64, 1>::shape(), vec![child])
    });
    let stale = output
        .member(store.bindings(), 1)
        .ok_or(BindError::ShapeMismatch)?;
    store.remove_shaped(root, 1, at(2), &mut Quiet);
    store.begin_cycle(at(3));
    store.get_or_create_with(root, 2, at(3), &mut Quiet, |store, owner| {
        let child = store.add_output::<i64>(owner).id();
        store.add_prepared_output(owner, Fixed::<i64, 1>::shape(), vec![child])
    });
    let rejected = std::panic::catch_unwind(|| stale.index(store.bindings(), 0));
    assert!(rejected.is_err());
    Ok(())
}
