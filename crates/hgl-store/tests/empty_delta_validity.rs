//! Empty sparse application changes parent validity without fabricating child data.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{Kind, OutputId, Store, Wake};
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
fn time(n: i64) -> EngineTime {
    EngineTime::from_micros(n)
}
fn empty(store: &mut Store, output: OutputId, n: i64, wakes: &mut Wakes) {
    let generation = store.bindings().output(output).generation;
    store
        .prepared()
        .tick(time(n), NodeId(0), wakes)
        .initialize_sparse(output, generation);
}
#[test]
fn initial_repeat_and_revalidation_preserve_children_times_and_allocations() {
    let scalar = Kind::Ts(ScalarType::I64);
    for kind in [
        Kind::Set(ScalarType::I64),
        Kind::Dictionary(Box::new(scalar.clone())),
        Kind::Growing(Box::new(scalar.clone())),
        Kind::List(Box::new(scalar.clone()), 2),
        Kind::Bundle(vec![("value".into(), scalar)]),
        Kind::List(Box::new(Kind::Ts(ScalarType::I64)), 0),
        Kind::Bundle(vec![]),
    ] {
        let mut store = Store::new();
        let output = store.add_shaped_output(NodeId(0), kind.clone());
        let input = store.add_shaped_input(NodeId(1), kind, true);
        store.bind(input, output).unwrap();
        store.prepare_collection_inputs();
        let zero_children = store.bindings().output(output).fixed.is_empty();
        let mut wakes = Wakes::default();
        let ((), allocations) = count_in(|| {
            assert!(!store.input_valid(input));
            empty(&mut store, output, 1, &mut wakes);
            empty(&mut store, output, 1, &mut wakes);
            assert!(store.input_valid(input));
            assert!(store.bindings().modified(input, time(1)));
            assert_eq!(store.bindings().all_valid(input), zero_children);
            assert!(
                store
                    .bindings()
                    .output(output)
                    .fixed
                    .iter()
                    .all(|child| store.bindings().output(*child).modified_at == EngineTime::NEVER)
            );
            store.begin_cycle(time(2));
            empty(&mut store, output, 2, &mut wakes);
            assert!(!store.bindings().modified(input, time(2)));
            assert_eq!(store.bindings().last_modified(input), time(1));
            store.invalidate(output, time(3), &mut wakes);
            assert!(!store.input_valid(input));
            assert_eq!(store.bindings().last_modified(input), EngineTime::NEVER);
            empty(&mut store, output, 4, &mut wakes);
            assert!(store.input_valid(input));
            assert!(store.bindings().modified(input, time(4)));
            assert_eq!(store.bindings().last_modified(input), time(4));
        });
        assert_eq!(allocations, 0);
        assert_eq!(wakes.0, 3);
    }
}
#[test]
fn targeted_empty_children_tick_only_their_ancestors() {
    let child = Kind::List(Box::new(Kind::Ts(ScalarType::I64)), 2);
    let kind = Kind::Bundle(vec![
        ("left".into(), child.clone()),
        ("right".into(), child),
    ]);
    let mut store = Store::new();
    let output = store.add_shaped_output(NodeId(0), kind.clone());
    let input = store.add_shaped_input(NodeId(1), kind, true);
    store.bind(input, output).unwrap();
    let left = store.bindings().fixed_output(output, 0);
    let right = store.bindings().fixed_output(output, 1);
    let view = store.bindings().fixed_input(input, 0);
    let mut wakes = Wakes::default();
    empty(&mut store, left, 1, &mut wakes);
    assert!(store.input_valid(input));
    assert!(store.input_valid(view));
    assert!(!store.bindings().all_valid(input));
    assert!(!store.bindings().all_valid(view));
    store.begin_cycle(time(2));
    empty(&mut store, left, 2, &mut wakes);
    empty(&mut store, output, 2, &mut wakes);
    assert!(!store.bindings().modified(input, time(2)));
    assert_eq!(store.bindings().last_modified(input), time(1));
    empty(&mut store, right, 3, &mut wakes);
    assert!(store.bindings().all_valid(input));
    assert_eq!(store.bindings().last_modified(input), time(3));
    assert_eq!(store.bindings().last_modified(view), time(1));
    store.invalidate(output, time(4), &mut wakes);
    empty(&mut store, output, 5, &mut wakes);
    assert!(store.input_valid(input));
    assert!(!store.input_valid(view));
    assert!(!store.bindings().all_valid(input));
    assert_eq!(store.bindings().last_modified(view), EngineTime::NEVER);
}
