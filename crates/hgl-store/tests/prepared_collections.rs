//! Finite collection domains allocate all member storage and projections before start.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{Kind, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[derive(Default)]
struct Wakes(usize);
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {
        self.0 += 1;
    }
}
fn time(tick: i64) -> EngineTime {
    EngineTime::from_micros(tick)
}
#[test]
fn first_remove_reinsert_preserves_absence_generations_and_sparse_child_validity_without_allocating()
 {
    let mut store = Store::new();
    let fields = Kind::Bundle(vec![
        ("a".into(), Kind::Ts(ScalarType::I64)),
        ("b".into(), Kind::Ts(ScalarType::I64)),
    ]);
    let kind = Kind::Dictionary(Box::new(fields.clone()));
    let root = store.add_shaped_output(NodeId(0), kind.clone());
    let keys = (0..128).collect::<Vec<_>>();
    store.prepare_collection(root, &keys, |store, owner| {
        store.add_shaped_output(owner, fields.clone())
    });
    let input = store.add_shaped_input(NodeId(1), kind, true);
    store.bind(input, root).unwrap_or_else(|_| unreachable!());
    store.prepare_collection_inputs();
    assert_eq!(store.bindings().keys(input).count(), 0);
    assert!(!store.input_valid(input));
    assert!(!store.bindings().all_valid(input));
    let counts = store.bindings().storage_counts();
    let mut wakes = Wakes::default();
    let ((), allocations) = count_in(|| {
        let child = store.get_or_create_shaped(root, 7, time(1), &mut wakes);
        let a = store
            .scalar_output::<i64>(store.bindings().fixed_output(child, 0))
            .unwrap_or_else(|_| unreachable!());
        let b = store
            .scalar_output::<i64>(store.bindings().fixed_output(child, 1))
            .unwrap_or_else(|_| unreachable!());
        let reference = store.reference(child);
        store.set(a, 1, time(1), NodeId(0), &mut wakes);
        store.set(b, 2, time(1), NodeId(0), &mut wakes);
        let view = store
            .bindings()
            .child_input(input, 7)
            .unwrap_or_else(|| unreachable!());
        let b_view = store
            .scalar_input::<i64>(store.bindings().fixed_input(view, 1))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(store.get(b_view), 2);
        store.remove_shaped(root, 7, time(2), &mut wakes);
        assert!(store.bindings().child_input(input, 7).is_none());
        assert_eq!(store.bindings().removed_input(input, 7), Some(view));
        assert_eq!(store.get(b_view), 2);
        store.begin_cycle(time(3));
        assert!(store.bindings().resolve(reference).is_none());
        assert!(store.output_value(b).is_none());
        assert!(!store.valid(b_view));
        for cycle in 0..30 {
            let now = time(3 + cycle * 2);
            let next = store.get_or_create_shaped(root, 7, now, &mut wakes);
            assert_eq!(next, child);
            let a = store
                .scalar_output::<i64>(store.bindings().fixed_output(next, 0))
                .unwrap_or_else(|_| unreachable!());
            store.set(a, 3 + cycle, now, NodeId(0), &mut wakes);
            assert!(!store.valid(b_view));
            assert!(!store.bindings().all_valid(view));
            assert_eq!(store.bindings().keys(input).count(), 1);
            store.remove_shaped(root, 7, time(4 + cycle * 2), &mut wakes);
        }
        store.begin_cycle(time(64));
    });
    assert_eq!(allocations, 0);
    assert_eq!(store.bindings().storage_counts(), counts);
    assert_eq!(wakes.0, 62);
}
#[test]
fn nested_domains_keep_never_used_keys_invisible_and_reset_descendant_membership() {
    let mut store = Store::new();
    let leaf = Kind::Ts(ScalarType::I64);
    let inner = Kind::Dictionary(Box::new(leaf));
    let outer = Kind::Dictionary(Box::new(inner.clone()));
    let root = store.add_shaped_output(NodeId(0), outer.clone());
    store.prepare_collection(root, &[1, 2, 3], |store, owner| {
        let child = store.add_shaped_output(owner, inner.clone());
        store.prepare_collection(child, &[10, 11, 12], |store, owner| {
            store.add_output::<i64>(owner).id()
        });
        child
    });
    let input = store.add_shaped_input(NodeId(1), outer, true);
    store.bind(input, root).unwrap_or_else(|_| unreachable!());
    store.prepare_collection_inputs();
    let counts = store.bindings().storage_counts();
    let mut wakes = Wakes::default();
    let ((), allocations) = count_in(|| {
        for cycle in 0..25 {
            let now = time(1 + cycle * 2);
            let map = store.get_or_create_shaped(root, 1, now, &mut wakes);
            let view = store
                .bindings()
                .child_input(input, 1)
                .unwrap_or_else(|| unreachable!());
            assert_eq!(store.bindings().keys(view).count(), 0);
            let child = store.get_or_create_shaped(map, 10, now, &mut wakes);
            let output = store
                .scalar_output::<i64>(child)
                .unwrap_or_else(|_| unreachable!());
            store.set(output, cycle, now, NodeId(0), &mut wakes);
            assert_eq!(store.bindings().keys(input).count(), 1);
            assert_eq!(store.bindings().keys(view).count(), 1);
            store.remove_shaped(root, 1, time(2 + cycle * 2), &mut wakes);
        }
        store.begin_cycle(time(51));
    });
    assert_eq!(allocations, 0);
    assert_eq!(store.bindings().storage_counts(), counts);
    assert_eq!(wakes.0, 50);
}

fn owning_key_cycles<K: hgl_store::Key>(value: &K::Value, ty: ScalarType) -> hgl_types::NodeResult {
    let mut store = Store::new();
    <K as hgl_store::Key>::prepare(&mut store.keys, value)?;
    let domain = K::ids(&store.keys).to_vec();
    let kind = Kind::KeyedDictionary(
        hgl_types::OrdinaryType::Scalar(ty),
        Box::new(Kind::Ts(ScalarType::I64)),
    );
    let root = store.add_shaped_output(NodeId(0), kind.clone());
    store.prepare_collection(root, &domain, |store, owner| {
        store.add_output::<i64>(owner).id()
    });
    let input = store.add_shaped_input(NodeId(1), kind, true);
    store.bind(input, root).unwrap_or_else(|_| unreachable!());
    store.prepare_collection_inputs();
    let counts = store.bindings().storage_counts();
    let mut wakes = Wakes::default();
    let (result, allocations) = count_in(|| -> hgl_types::NodeResult {
        for cycle in 0..30 {
            let now = time(1 + cycle * 2);
            let key = K::id(&store.keys, value)?;
            let child = store.get_or_create_shaped(root, key, now, &mut wakes);
            let output = store
                .scalar_output::<i64>(child)
                .unwrap_or_else(|_| unreachable!());
            store.set(output, cycle, now, NodeId(0), &mut wakes);
            let view = store
                .bindings()
                .child_input(input, key)
                .unwrap_or_else(|| unreachable!());
            assert_eq!(
                store.get(
                    store
                        .scalar_input::<i64>(view)
                        .unwrap_or_else(|_| unreachable!())
                ),
                cycle
            );
            assert_eq!(store.bindings().keys(input).count(), 1);
            store.remove_shaped(root, key, time(2 + cycle * 2), &mut wakes);
            assert_eq!(store.bindings().keys(input).count(), 0);
        }
        store.begin_cycle(time(61));
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(counts, store.bindings().storage_counts());
    assert_eq!(wakes.0, 60);
    Ok(())
}
#[test]
fn string_and_provider_owning_keys_publish_remove_and_reinsert_without_allocating()
-> Result<(), Box<dyn std::error::Error>> {
    use hgl_literals::{Literal, TemporalLiteral};
    let mut context = hgl_time_context::RunContext::from_bundled()?;
    let Literal::TimeZone(zone) =
        context.materialize(&TemporalLiteral::TimeZone("US/Eastern".into()))?
    else {
        unreachable!()
    };
    let Literal::ZonedTime(clock) = context.materialize(&TemporalLiteral::ZonedTime {
        time_micros: 34_200_123_456,
        zone: "US/Eastern".into(),
    })?
    else {
        unreachable!()
    };
    drop(context);
    owning_key_cycles::<String>(&"independently owned string".into(), ScalarType::Text)
        .map_err(|error| format!("{error:?}"))?;
    owning_key_cycles::<hgl_types::ZoneId>(&zone, ScalarType::TimeZone)
        .map_err(|error| format!("{error:?}"))?;
    owning_key_cycles::<hgl_types::ZonedTime>(&clock, ScalarType::ZonedTime)
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}
#[test]
fn runtime_key_pool_retains_removal_identity_and_prebound_projections() {
    let mut store = Store::new();
    let shape = Kind::Dictionary(Box::new(Kind::Ts(ScalarType::I64)));
    let root = store.add_shaped_output(NodeId(0), shape.clone());
    store.prepare_collection(root, &[0, 1, 2], |store, owner| {
        store.add_output::<i64>(owner).id()
    });
    store.prepared().bindings.prepare_pool(root);
    let input = store.add_shaped_input(NodeId(1), shape, true);
    store.bind(input, root).unwrap_or_else(|_| unreachable!());
    store.prepare_collection_inputs();
    let counts = store.bindings().storage_counts();
    let mut wake = Wakes::default();
    let ((), allocations) = count_in(|| {
        let first = store.get_or_create_shaped(root, i64::MIN, time(1), &mut wake);
        let second = store.get_or_create_shaped(root, i64::MAX, time(1), &mut wake);
        assert_ne!(first, second);
        let held = store.reference(first);
        let view = store
            .bindings()
            .child_input(input, i64::MIN)
            .unwrap_or_else(|| unreachable!());
        store.remove_shaped(root, i64::MIN, time(2), &mut wake);
        assert_eq!(store.bindings().removed_input(input, i64::MIN), Some(view));
        store.begin_cycle(time(3));
        assert!(store.bindings().resolve(held).is_none());
        let again = store.get_or_create_shaped(root, i64::MIN, time(3), &mut wake);
        assert_eq!(first, again);
        assert_eq!(store.bindings().child_input(input, i64::MIN), Some(view));
        let third = store.get_or_create_shaped(root, 16, time(3), &mut wake);
        assert_ne!(first, third);
        assert_ne!(second, third);
        assert_eq!(store.bindings().keys(input).count(), 3);
    });
    assert_eq!(allocations, 0);
    assert_eq!(store.bindings().storage_counts(), counts);
}
