//! Dense nested ticks and previously constructed REF routes allocate nothing.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{BindError, Kind, Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}

#[test]
fn warmed_nested_ticks_and_reference_switches_allocate_nothing() -> Result<(), BindError> {
    let mut store = Store::new();
    let shape = Kind::List(
        Box::new(Kind::Bundle(vec![
            ("left".into(), Kind::Scalar(ScalarType::I64)),
            ("right".into(), Kind::Scalar(ScalarType::Bool)),
        ])),
        2,
    );
    let a = store.add_shaped_output(NodeId(0), shape.clone());
    let b = store.add_shaped_output(NodeId(0), shape.clone());
    let r = store.add_shaped_output(NodeId(1), Kind::Reference(Box::new(shape.clone())));
    let input = store.add_shaped_input(NodeId(2), shape.clone(), true);
    store.follow(input, r, EngineTime::MIN_START, &mut Quiet)?;
    let items = store.items_reference(
        shape,
        vec![
            store.reference(store.bindings().fixed_output(a, 0)),
            store.reference(store.bindings().fixed_output(b, 1)),
        ],
    )?;
    let routes = [
        store.reference(a),
        store.reference(b),
        items,
        Reference::default(),
    ];
    let ai = store
        .bindings()
        .fixed_output(store.bindings().fixed_output(a, 0), 0);
    let bi = store
        .bindings()
        .fixed_output(store.bindings().fixed_output(b, 1), 0);
    let a_leaf = store.scalar_output::<i64>(ai)?;
    let b_leaf = store.scalar_output::<i64>(bi)?;
    let leaf = store
        .bindings()
        .fixed_input(store.bindings().fixed_input(input, 0), 0);
    let read = store.scalar_input::<i64>(leaf)?;
    let mut cycle = |n: usize| -> Result<(), BindError> {
        let now = EngineTime::from_micros(i64::try_from(n).unwrap_or_else(|_| unreachable!()) + 1);
        store.begin_cycle(now);
        store.set(a_leaf, 7, now, NodeId(0), &mut Quiet);
        store.set(b_leaf, 9, now, NodeId(0), &mut Quiet);
        store.set_reference(r, routes[n % 4], now, &mut Quiet)?;
        if store.valid(read) {
            assert_eq!(store.get(read), 7);
        }
        Ok(())
    };
    for n in 0..20 {
        cycle(n)?;
    }
    let (result, allocations) = count_in(|| -> Result<(), BindError> {
        for n in 20..10_020 {
            cycle(n)?;
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    Ok(())
}
