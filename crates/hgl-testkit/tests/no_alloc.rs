//! The harness allocates at instantiation and not in a cycle (card, "Speed").
//! In a file of its own because it replaces the global allocator.

use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_describe::{Builder, Registry, instantiate};
use hgl_kernel::Graph;
use hgl_store::Store;
use hgl_testkit::{Record, Replay};
use hgl_types::{EngineTime, NodeId, ScalarValue};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const WARM_UP: usize = 10;
const CYCLES: usize = 1_000;

#[test]
fn a_thousand_cycles_of_the_harness_allocate_nothing() {
    let total = WARM_UP + CYCLES;
    let mut registry = Registry::new();
    hgl_proto_nodes::register_all(&mut registry).unwrap();
    registry.register::<Replay>().unwrap();
    registry.register::<Record>().unwrap();

    let mut builder = Builder::new("replay, add one, record", &registry);
    let slot = ScalarValue::I64(0);
    let replay = builder.node("testkit.replay", &[("slot", slot)]).unwrap();
    let add_one = builder.node("add_one", &[]).unwrap();
    let cycles = ScalarValue::I64(i64::try_from(total).unwrap());
    let record = builder
        .node("testkit.record", &[("cycles", cycles)])
        .unwrap();
    builder.connect(replay, add_one, "in").unwrap();
    builder.connect(add_one, record, "in").unwrap();
    let description = builder.finish().unwrap();

    let mut store = Store::new();
    let mut graph = instantiate(&description, &registry, &mut store).unwrap();
    let ticks: Vec<Option<ScalarValue>> = (0..total)
        .map(|tick| Some(ScalarValue::I64(i64::try_from(tick).unwrap())))
        .collect();
    graph.node_mut::<Replay>(NodeId(0)).unwrap().load(&ticks);
    graph.start(&mut store, EngineTime::MIN_START).unwrap();

    let mut cycle = |graph: &mut Graph| {
        let now = graph.next_scheduled_time();
        assert_ne!(now, EngineTime::FOREVER);
        graph.evaluate(&mut store, now)
    };
    for _ in 0..WARM_UP {
        assert_eq!(cycle(&mut graph), Ok(()));
    }
    let (failures, allocations) =
        count_in(|| (0..CYCLES).filter(|_| cycle(&mut graph).is_err()).count());
    assert_eq!((failures, allocations), (0, 0));

    // Zero allocations would also be what doing nothing costs.
    let record = graph.node::<Record>(NodeId(2)).unwrap();
    let last = ScalarValue::I64(i64::try_from(total).unwrap());
    assert_eq!(record.seen().len(), total);
    assert_eq!(record.seen()[total - 1], Some(last));
}
